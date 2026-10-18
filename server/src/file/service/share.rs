use crate::{
    db::files::ShareRepository,
    error::AppError,
    file::{
        model::{FileContentPreview, FileEntry, FileKind, FileView, PreviewSize, SharingMode},
        storage::Storage,
        tree::FileTree,
    },
};
use std::path::PathBuf;
use tracing::instrument;
use uuid::Uuid;

/// Public, unauthenticated access to files that have been explicitly shared.
#[derive(Clone)]
pub struct ShareService {
    share: ShareRepository,
    storage: Storage,
}

impl ShareService {
    pub fn new(share: ShareRepository, storage: Storage) -> Self {
        Self { share, storage }
    }

    /// Metadata for a shared file or directory.
    #[instrument(skip(self), err(Debug))]
    pub async fn get(&self, id: Uuid) -> Result<FileView, AppError> {
        let file = self
            .share
            .get_shared(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        Ok(FileView::from(file))
    }

    /// Children of a shared directory. The directory itself must be shared.
    #[instrument(skip(self), err(Debug))]
    pub async fn list(&self, parent_id: Uuid) -> Result<Vec<FileView>, AppError> {
        let parent = self
            .share
            .get_shared(parent_id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        if parent.kind != FileKind::Directory {
            return Err(AppError::Unprocessable(
                "Only directories can be listed".into(),
            ));
        }

        self.share.list_shared_children(parent_id).await
    }

    /// Blob path for inline/stream access of a shared file (view or download mode).
    #[instrument(skip(self), err(Debug))]
    pub async fn content(
        &self,
        id: Uuid,
    ) -> Result<(FileEntry, PathBuf, &'static str), AppError> {
        self.shared_blob(id).await
    }

    /// Attachment download. Rejected when the share is view-only.
    ///
    /// Files stream as-is; directories are returned as a zip streamed while being built.
    #[instrument(skip(self), err(Debug))]
    pub async fn download(
        &self,
        id: Uuid,
    ) -> Result<(String, super::DownloadBody), AppError> {
        let file = self
            .share
            .get_shared(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        if file.sharing_mode != Some(SharingMode::Download) {
            return Err(AppError::Forbidden(
                "This share does not allow downloads".into(),
            ));
        }

        match file.kind {
            FileKind::File => {
                let hash = file.hash.as_deref().ok_or(AppError::Internal)?;
                let path = self.storage.path(hash);

                if !tokio::fs::try_exists(&path).await? {
                    tracing::error!(%id, hash, "shared file row has no blob on disk");
                    return Err(AppError::Internal);
                }

                let mime = file
                    .file_type
                    .as_ref()
                    .map(|file_type| file_type.mime())
                    .unwrap_or("application/octet-stream");

                Ok((file.name, super::DownloadBody::File { path, mime }))
            }
            FileKind::Directory => {
                let children = self
                    .share
                    .list_shared_descendants(file.root_id, file.id)
                    .await?;

                let blob_paths = super::blob_paths_for(&self.storage, &children).await?;
                let name = format!("{}.zip", file.name);

                let tree = FileTree {
                    id: file.id,
                    name: file.name,
                    kind: file.kind,
                    file_type: file.file_type,
                    metadata: file.metadata.into_option(),
                    children: FileTree::from_entries(children),
                };

                Ok((name, super::zip_directory_stream(tree, blob_paths)))
            }
        }
    }

    async fn shared_blob(
        &self,
        id: Uuid,
    ) -> Result<(FileEntry, PathBuf, &'static str), AppError> {
        let file = self
            .share
            .get_shared(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        if file.kind != FileKind::File {
            return Err(AppError::Unprocessable(
                "Directories cannot be streamed".into(),
            ));
        }

        let hash = file.hash.as_deref().ok_or(AppError::Internal)?;

        let path = self.storage.path(hash);

        if !tokio::fs::try_exists(&path).await? {
            tracing::error!(%id, hash, "shared file row has no blob on disk");
            return Err(AppError::Internal);
        }

        let mime = file
            .file_type
            .as_ref()
            .map(|file_type| file_type.mime())
            .unwrap_or("application/octet-stream");

        Ok((file, path, mime))
    }

    /// Generated preview icon for a shared file, when one exists.
    #[instrument(skip(self), fields(?size), err(Debug))]
    pub async fn preview(
        &self,
        id: Uuid,
        size: PreviewSize,
    ) -> Result<FileContentPreview, AppError> {
        let file = self
            .share
            .get_shared(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        if file.kind != FileKind::File {
            return Err(AppError::Unprocessable(
                "Directories do not have previews".into(),
            ));
        }

        if file.metadata.is_empty() {
            return Err(AppError::ResourceMissing("Preview does not exist".into()));
        }

        let hash = match size {
            PreviewSize::Icon => file.metadata.preview_64.as_deref(),
            PreviewSize::Tile => file.metadata.preview_256.as_deref(),
        }
        .ok_or_else(|| AppError::ResourceMissing("Preview does not exist".into()))?;

        let path = self.storage.preview_path(hash);

        FileContentPreview::new(path, "image/webp").await
    }
}
