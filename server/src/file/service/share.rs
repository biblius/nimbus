use crate::{
    db::files::ShareRepository,
    error::AppError,
    file::{
        model::{FileContentPreview, FileEntry, FileKind, FileView, PreviewSize},
        storage::Storage,
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

    /// Blob path for a shared file download.
    #[instrument(skip(self), err(Debug))]
    pub async fn content(
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
                "Directories cannot be downloaded".into(),
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

        let meta = file
            .metadata
            .as_ref()
            .ok_or_else(|| AppError::ResourceMissing("Preview does not exist".into()))?;

        let hash = match size {
            PreviewSize::Icon => meta.preview_64.as_deref(),
            PreviewSize::Tile => meta.preview_256.as_deref(),
        }
        .ok_or_else(|| AppError::ResourceMissing("Preview does not exist".into()))?;

        let path = self.storage.preview_path(hash);

        FileContentPreview::new(path, "image/webp").await
    }
}
