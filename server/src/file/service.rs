use super::{
    format::FileFormat,
    model::{CreateDirectory, FileEntry, FileKind},
    storage::{Storage, TempFile, UploadWriter},
};
use crate::{
    db::{
        Db,
        files::{FileRepository, NewFile},
        tx::Transactor,
        user_folders::UserFolderRepository,
    },
    error::AppError,
    file::model::RootView,
    user::model::{Role, User},
};
use chrono::{Duration, Utc};
use sqlx::Transaction;
use std::collections::VecDeque;
use tracing::instrument;
use uuid::Uuid;
use validify::Validify;

#[derive(Clone)]
pub struct FileService {
    files: FileRepository,
    folders: UserFolderRepository,
    storage: Storage,
    retention_days: u16,
    tx: Transactor,
}

impl FileService {
    pub fn new(
        files: FileRepository,
        folders: UserFolderRepository,
        storage: Storage,
        retention_days: u16,
        tx: Transactor,
    ) -> Self {
        Self {
            files,
            folders,
            storage,
            retention_days,
            tx,
        }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn prepare(&self) -> Result<(), AppError> {
        self.storage.prepare().await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn list_roots(&self, user: &User) -> Result<Vec<RootView>, AppError> {
        self.folders.list_roots(user.id).await
    }

    /// Bytes stored in a root. Directories are not counted.
    #[instrument(skip(self, user), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn usage(&self, user: &User, root_id: Uuid) -> Result<i64, AppError> {
        if self.files.get_root(root_id).await?.is_none() {
            return Err(AppError::ResourceMissing("folder does not exist".into()));
        }

        if user.role != Role::Admin && !self.folders.can_access(user.id, root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        self.files.root_quota_usage(root_id).await
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn list_directory(
        &self,
        user: &User,
        root_id: Uuid,
        dir_id: Option<Uuid>,
    ) -> Result<Vec<FileEntry>, AppError> {
        if !self.folders.can_access(user.id, root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }
        self.files.list_files(root_id, dir_id).await
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

        let name = self.writable(user, root, input.parent, &input.name).await?;

        let entry = self
            .files
            .create(NewFile {
                id: Uuid::new_v4(),
                root_id: root,
                created_by: user.id,
                kind: FileKind::Directory,
                name,
                parent: input.parent,
                family: None,
                format: None,
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
        let name = self.writable(user, root, parent, &name).await?;
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

        let format = FileFormat::from_ext(&file.name);
        let family = format.family();

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
                    family: Some(family),
                    format: Some(format),
                    hash: Some(file.hash.clone()),
                    size_bytes: file.size,
                },
            )
            .await?;

        self.storage.persist(&mut file).await?;

        tx.commit().await?;

        Ok(entry)
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn content(
        &self,
        user: &User,
        id: Uuid,
    ) -> Result<(FileEntry, std::path::PathBuf, &'static str), AppError> {
        let file = self.readable(user, id).await?;

        if file.kind != FileKind::File {
            return Err(AppError::Unprocessable(
                "directories cannot be downloaded".into(),
            ));
        }

        let hash = file.hash.as_deref().ok_or(AppError::Internal)?;

        let path = self.storage.path(hash);

        if !tokio::fs::try_exists(&path).await? {
            tracing::error!(%id, hash, "file row has no blob on disk");
            return Err(AppError::Internal);
        }

        let mime = file
            .format
            .as_ref()
            .map(|format| format.mime())
            .unwrap_or("application/octet-stream");

        Ok((file, path, mime))
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    pub async fn delete(&self, user: &User, id: Uuid) -> Result<(), AppError> {
        let file = self.readable(user, id).await?;
        self.files.soft_delete(&[file.id]).await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn purge_expired(&self) -> Result<(), AppError> {
        let cutoff = Utc::now() - Duration::days(self.retention_days as i64);
        self.purge_deleted_before(cutoff).await
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn purge_deleted_before(
        &self,
        cutoff: chrono::DateTime<Utc>,
    ) -> Result<(), AppError> {
        let mut tx = self.tx.begin().await?;

        let pending = self.files.list_pending_deletion(&mut *tx, cutoff).await?;

        let tx = Self::delete_recursive(&self.files, &self.storage, tx, pending).await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn delete_recursive<'tx>(
        files: &FileRepository,
        storage: &Storage,
        mut tx: Transaction<'tx, Db>,
        entries: Vec<FileEntry>,
    ) -> Result<Transaction<'tx, Db>, AppError> {
        let mut ids = vec![];

        let mut queue = VecDeque::from(entries);

        while let Some(file) = queue.pop_front() {
            ids.push(file.id);

            if file.kind != FileKind::Directory {
                continue;
            }

            let children = files.list_all_children(&mut *tx, file.id).await?;

            for child in children {
                queue.push_back(child);
            }
        }

        tracing::info!("removing {} files that are pending deletion", ids.len());

        let deleted = files.delete(&mut *tx, &ids).await?;

        for hash in deleted {
            let remaining = files.count_hash(&mut *tx, &hash).await?;
            if remaining == 0 {
                match storage.remove(&hash).await {
                    Ok(()) => {
                        tracing::info!("deleted file {hash}")
                    }
                    Err(e) => {
                        tracing::error!(error=%e,"unable to delete file: {hash}");
                        return Err(e);
                    }
                }
            }
        }

        Ok(tx)
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    async fn readable(&self, user: &User, id: Uuid) -> Result<FileEntry, AppError> {
        let file = self
            .files
            .get_live(id)
            .await?
            .ok_or_else(|| AppError::ResourceMissing("file does not exist".into()))?;

        if user.role != Role::Admin && !self.folders.can_access(user.id, file.root_id).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        Ok(file)
    }

    #[instrument(skip(self, user), fields(user_id = %user.id), err(Debug))]
    async fn writable(
        &self,
        user: &User,
        root: Uuid,
        parent: Option<Uuid>,
        name: &str,
    ) -> Result<String, AppError> {
        let name = FileEntry::valid_name(name)?.to_string();

        if user.role != Role::Admin && !self.folders.can_access(user.id, root).await? {
            return Err(AppError::Forbidden(
                "You do not have access to this folder".into(),
            ));
        }

        if self.files.name_taken(root, parent, &name).await? {
            return Err(AppError::ResourceConflict(
                "A directory with that name already exists".into(),
            ));
        }

        Ok(name)
    }
}
