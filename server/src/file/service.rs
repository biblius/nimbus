use super::{
    model::{CreateDirectory, FileEntry, FileKind},
    storage::{Storage, TempFile, UploadWriter},
};
use crate::{
    db::{
        Db,
        files::{FileRepository, FileTable, NewFile},
        tx::Transactor,
        user_folders::UserFolderRepository,
    },
    error::AppError,
    file::{
        model::{RootView, TrashFiles},
        tree::FileTree,
    },
    user::model::{Role, User},
};
use chrono::{Duration, Utc};
use sqlx::{Transaction, migrate::MigrateError::Source};
use std::collections::{HashMap, HashSet};
use tracing::instrument;
use uuid::Uuid;
use validify::{Validate, Validify};

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
    ) -> Result<Vec<FileEntry>, AppError> {
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
            children: FileTree::from_entries(files),
        })
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

        let deleted = self.files.delete_trashed_before(&mut *tx, cutoff).await?;

        let tx = Self::remove_unreferenced(&self.files, &self.storage, tx, deleted).await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn delete_with_descendants<'tx>(
        files: &FileRepository,
        storage: &Storage,
        mut tx: Transaction<'tx, Db>,
        table: FileTable,
        ids: &[Uuid],
    ) -> Result<Transaction<'tx, Db>, AppError> {
        tracing::info!(
            roots = ids.len(),
            "removing file trees that are pending deletion"
        );

        let deleted = files.delete_tree(&mut *tx, table, ids).await?;

        Self::remove_unreferenced(files, storage, tx, deleted).await
    }

    async fn remove_unreferenced<'tx>(
        files: &FileRepository,
        storage: &Storage,
        mut tx: Transaction<'tx, Db>,
        hashes: Vec<String>,
    ) -> Result<Transaction<'tx, Db>, AppError> {
        for hash in hashes {
            let remaining = files.count_hash(&mut *tx, &hash).await?;

            if remaining > 0 {
                tracing::info!(%remaining, "file {hash} is still in use");
                continue;
            }

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

        Ok(tx)
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
