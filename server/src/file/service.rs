use super::{
    model::{CreateDirectory, FileEntry, FileKind, FileView},
    queue::{ProcessingQueue, ProcessingQueueHandle},
    storage::{Storage, TempFile, UploadWriter},
};
use crate::{
    db::{
        MetadataRepository,
        files::{FileRepository, NewFile},
        tx::Transactor,
        user_folders::UserFolderRepository,
    },
    error::AppError,
    file::{
        format::FileType,
        model::{FileContentPreview, RestoreFile, RootView, TrashFiles},
        tree::FileTree,
    },
    user::model::{Role, User},
};
use std::{collections::HashSet, path::PathBuf};
use tracing::instrument;
use uuid::Uuid;
use validify::{Validate, Validify};

#[derive(Clone)]
pub struct FileService {
    files: FileRepository,
    folders: UserFolderRepository,
    metadata: MetadataRepository,
    storage: Storage,
    processing: ProcessingQueueHandle,
    tx: Transactor,
}

impl FileService {
    pub fn new(
        files: FileRepository,
        folders: UserFolderRepository,
        metadata: MetadataRepository,
        storage: Storage,
        processing: ProcessingQueueHandle,
        tx: Transactor,
    ) -> Self {
        Self {
            files,
            folders,
            metadata,
            storage,
            processing,
            tx,
        }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn prepare(&self) -> Result<(), AppError> {
        self.storage.prepare().await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn list_accessible_roots(&self, user: &User) -> Result<Vec<RootView>, AppError> {
        self.files.list_accessible_roots(user.id).await
    }

    /// Bytes stored in a root. Directories are not counted.
    #[instrument(skip(self, user), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn usage(&self, user: &User, root_id: Uuid) -> Result<i64, AppError> {
        if self.files.get_root(root_id).await?.is_none() {
            return Err(AppError::ResourceMissing("Folder does not exist".into()));
        }

        if user.role != Role::Admin && !self.folders.can_access(user.id, root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        self.files.root_quota_usage(root_id).await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn list(
        &self,
        user: &User,
        root_id: Uuid,
        dir_id: Option<Uuid>,
    ) -> Result<Vec<FileView>, AppError> {
        if !self.folders.can_access(user.id, root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }
        self.files.list_files(root_id, dir_id).await
    }

    /// Directories from the root down to `file_id`, including `file_id`.
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn ancestors(
        &self,
        user: &User,
        root_id: Uuid,
        file_id: Uuid,
    ) -> Result<Vec<FileEntry>, AppError> {
        if !self.folders.can_access(user.id, root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        let mut chain = Vec::new();
        let mut current = Some(file_id);
        let mut seen = HashSet::new();

        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(AppError::Unprocessable(
                    "Directory tree contains a cycle".into(),
                ));
            }

            let entry = self
                .files
                .get_live(id)
                .await?
                .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

            if entry.root_id != root_id {
                return Err(AppError::ResourceMissing("File does not exist".into()));
            }

            current = entry.parent;

            chain.push(entry);
        }

        chain.reverse();

        Ok(chain)
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn metadata(&self, user: &User, id: Uuid) -> Result<FileEntry, AppError> {
        self.readable(user, id).await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn create_directory(
        &self,
        user: &User,
        root: Uuid,
        mut input: CreateDirectory,
    ) -> Result<FileEntry, AppError> {
        input.validify()?;

        let name = self.writable(user, root, &input.name).await?;

        let entry = self
            .files
            .create(NewFile {
                id: Uuid::new_v4(),
                root_id: root,
                created_by: user.id,
                kind: FileKind::Directory,
                name,
                parent: input.parent,
                file_type: None,
                hash: None,
                size_bytes: 0,
            })
            .await?;

        Ok(entry)
    }

    /// Validate the parameters of a file upload and return the writer if valid.
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn start_upload(
        &self,
        user: &User,
        root: Uuid,
        parent: Option<Uuid>,
        name: String,
    ) -> Result<UploadWriter, AppError> {
        let name = self.writable(user, root, &name).await?;
        self.storage.writer(name).await
    }

    /// Store the uploaded bytes, then insert the fully-described file row.
    #[instrument(skip(self, user, file), fields(user_id = %user.id, name = %file.name), err(Debug))]
    pub async fn finish_upload(
        &self,
        user: &User,
        root_id: Uuid,
        parent: Option<Uuid>,
        mut file: TempFile,
    ) -> Result<FileEntry, AppError> {
        let quota = self.files.root_quota(root_id).await?;

        if quota > 0 {
            let used = self.files.root_quota_usage(root_id).await?;

            if used.saturating_add(file.size) > quota {
                return Err(AppError::QuotaExceeded);
            }
        }

        let mut tx = self.tx.begin().await?;

        let entry = self
            .files
            .insert(
                &mut *tx,
                NewFile {
                    id: Uuid::new_v4(),
                    root_id,
                    created_by: user.id,
                    kind: FileKind::File,
                    name: file.name.clone(),
                    parent,
                    file_type: Some(file.file_type.clone()),
                    hash: Some(file.hash.clone()),
                    size_bytes: file.size,
                },
            )
            .await?;

        self.storage.persist(&mut file).await?;

        tx.commit().await?;

        if let Err(e) = ProcessingQueue::submit(&self.processing, entry.clone()).await {
            tracing::error!(file=%entry.id, error=%e, "failed to enqueue file for processing");
        }

        Ok(entry)
    }

    /// Open a writer that will replace an existing file's blob.
    #[instrument(skip(self, user), fields(user_id = %user.id, file_id = %file_id), err(Debug))]
    pub async fn start_overwrite(
        &self,
        user: &User,
        root_id: Uuid,
        file_id: Uuid,
    ) -> Result<(FileEntry, UploadWriter), AppError> {
        let file = self.readable(user, file_id).await?;

        if file.root_id != root_id || file.kind != FileKind::File {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        if file
            .file_type
            .as_ref()
            .is_none_or(|ty| !matches!(ty, FileType::Text(_)))
        {
            return Err(AppError::Unprocessable(
                "Cannot overwrite contents - not a text file".into(),
            ));
        }

        let writer = self.storage.writer(file.name.clone()).await?;

        Ok((file, writer))
    }

    /// Persist new bytes and point the existing file row at them.
    #[instrument(skip(self, user, existing, file), fields(user_id = %user.id, file_id = %existing.id, name = %file.name), err(Debug))]
    pub async fn finish_overwrite(
        &self,
        user: &User,
        existing: FileEntry,
        mut file: TempFile,
    ) -> Result<FileEntry, AppError> {
        let Some(old_hash) = existing.hash.as_deref() else {
            // Should never happen
            return Err(AppError::Unprocessable(
                "Original file is missing hash".into(),
            ));
        };

        let content_changed = old_hash != file.hash.as_str();

        if !content_changed {
            tracing::info!("file contents are the same, skipping overwrite");
            return Ok(existing);
        }

        let quota = self.files.root_quota(existing.root_id).await?;

        if quota > 0 {
            let used = self.files.root_quota_usage(existing.root_id).await?;

            if used
                .saturating_sub(existing.size_bytes)
                .saturating_add(file.size)
                > quota
            {
                return Err(AppError::QuotaExceeded);
            }
        }

        let mut tx = self.tx.begin().await?;

        let entry = self
            .files
            .update_content(
                &mut *tx,
                existing.id,
                &file.hash,
                file.size,
                &file.file_type,
            )
            .await?;

        self.storage.persist(&mut file).await?;

        tx.commit().await?;

        match self.files.count_hash(old_hash).await {
            Ok(count) => {
                if count == 0
                    && let Err(error) = self.storage.remove(old_hash).await
                {
                    tracing::error!(%error, hash = %old_hash, "failed to remove replaced blob");
                }
            }
            Err(error) => {
                tracing::error!(%error, hash = %old_hash, "failed to count replaced blob");
            }
        }

        Ok(entry)
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn content(
        &self,
        user: &User,
        id: Uuid,
    ) -> Result<(FileEntry, PathBuf, &'static str), AppError> {
        let file = self.readable(user, id).await?;

        if file.kind != FileKind::File {
            return Err(AppError::Unprocessable(
                "Directories cannot be downloaded".into(),
            ));
        }

        let hash = file.hash.as_deref().ok_or(AppError::Internal)?;

        let path = self.storage.path(hash);

        if !tokio::fs::try_exists(&path).await? {
            tracing::error!(%id, hash, "file row has no blob on disk");
            return Err(AppError::Internal);
        }

        let mime = file
            .file_type
            .as_ref()
            .map(|file_type| file_type.mime())
            .unwrap_or("application/octet-stream");

        Ok((file, path, mime))
    }

    /// Serve the generated preview image for a file, when one exists.
    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn preview(&self, user: &User, id: Uuid) -> Result<FileContentPreview, AppError> {
        let file = self.readable(user, id).await?;

        if file.kind != FileKind::File {
            return Err(AppError::Unprocessable(
                "Directories do not have previews".into(),
            ));
        }

        let meta = self
            .metadata
            .get(file.id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("Preview does not exist".into()))?;

        let hash = meta
            .preview
            .ok_or_else(|| AppError::ResourceMissing("Preview does not exist".into()))?;

        let path = self.storage.preview_path(&hash);

        let preview = FileContentPreview::new(path, "image/webp").await?;

        Ok(preview)
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn delete(&self, user: &User, id: Uuid) -> Result<(), AppError> {
        let file = self.readable(user, id).await?;
        self.files.trash(file.root_id, user.id, &[file.id]).await
    }

    /// Move every listed file in `root_id` to the trash, including descendants.
    #[instrument(skip(self, user, input), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn delete_many(
        &self,
        user: &User,
        root_id: Uuid,
        input: TrashFiles,
    ) -> Result<(), AppError> {
        input.validate()?;

        if user.role != Role::Admin && !self.folders.can_access(user.id, root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        self.files.trash(root_id, user.id, &input.ids).await
    }

    /// Move a trashed item and its descendants back into `files` under `input.parent`.
    #[instrument(skip(self, user, input), fields(user_id = %user.id, root_id = %root_id, file_id = %id), err(Debug))]
    pub async fn restore(
        &self,
        user: &User,
        root_id: Uuid,
        id: Uuid,
        input: RestoreFile,
    ) -> Result<(), AppError> {
        if user.role != Role::Admin && !self.folders.can_access(user.id, root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        let Some(trashed_root) = self.files.trashed_root(id).await? else {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        };

        if trashed_root != root_id {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        if let Some(parent) = input.parent {
            let entry =
                self.files.get_live(parent).await?.ok_or_else(|| {
                    AppError::Unprocessable("Target directory does not exist".into())
                })?;

            if entry.root_id != root_id || entry.kind != FileKind::Directory {
                return Err(AppError::Unprocessable("Target is not a directory".into()));
            }
        }

        let quota = self.files.root_quota(root_id).await?;

        let used = if quota > 0 {
            self.files.root_quota_usage(root_id).await?
        } else {
            0
        };

        let mut tx = self.tx.begin().await?;

        let mut restored = self
            .files
            .delete_trashed_tree(&mut *tx, root_id, id)
            .await?;

        let bytes: i64 = restored
            .iter()
            .filter(|row| row.kind == FileKind::File)
            .map(|row| row.size_bytes)
            .sum();

        if quota > 0 && used.saturating_add(bytes) > quota {
            return Err(AppError::QuotaExceeded);
        }

        for file in &mut restored {
            if file.id == id {
                file.parent = input.parent;
                break;
            }
        }

        self.files.insert_many(&mut *tx, restored).await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn get_trash_tree(&self, user: &User, root_id: Uuid) -> Result<FileTree, AppError> {
        if user.role != Role::Admin && !self.folders.can_access(user.id, root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        let files = self.files.list_trash(root_id).await?;

        Ok(FileTree {
            id: root_id,
            name: String::new(),
            kind: FileKind::Directory,
            children: FileTree::from_entries(files),
        })
    }

    pub async fn get_tree(
        &self,
        user: &User,
        root_id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<FileTree, AppError> {
        if user.role != Role::Admin && !self.folders.can_access(user.id, root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        let files = self.files.list_descendants(root_id, parent).await?;

        Ok(FileTree {
            id: parent.unwrap_or(root_id),
            name: String::new(),
            kind: FileKind::Directory,
            children: FileTree::from_entries(files),
        })
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    async fn readable(&self, user: &User, id: Uuid) -> Result<FileEntry, AppError> {
        let file = self
            .files
            .get_live(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        if user.role != Role::Admin && !self.folders.can_access(user.id, file.root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        Ok(file)
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    async fn writable(&self, user: &User, root: Uuid, name: &str) -> Result<String, AppError> {
        let name = FileEntry::valid_name(name)?.to_string();

        if user.role != Role::Admin && !self.folders.can_access(user.id, root).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        Ok(name)
    }
}
