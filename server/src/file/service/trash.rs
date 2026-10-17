use crate::{
    db::{
        files::{FileRepository, FileTable, TrashRepository},
        tx::Transactor,
        user_folders::UserFolderRepository,
    },
    error::AppError,
    file::{
        model::{FileKind, RestoreFile},
        queue::{DeletionQueue, DeletionQueueHandle, DeletionResult, PendingDeletion},
        tree::FileTree,
    },
    user::model::User,
};
use tokio::sync::mpsc;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct TrashFileService {
    trash: TrashRepository,
    files: FileRepository,
    folders: UserFolderRepository,
    tx: Transactor,
}

impl TrashFileService {
    pub fn new(
        trash: TrashRepository,
        files: FileRepository,
        folders: UserFolderRepository,
        tx: Transactor,
    ) -> Self {
        Self {
            trash,
            files,
            folders,
            tx,
        }
    }

    /// Permanently delete one trashed item and its descendants.
    #[instrument(skip(self, user, handle), fields(user_id = %user.id, root_id = %root_id, file_id = %id), err(Debug))]
    pub async fn delete_trash(
        &self,
        user: &User,
        root_id: Uuid,
        id: Uuid,
        handle: DeletionQueueHandle,
    ) -> Result<mpsc::Receiver<DeletionResult>, AppError> {
        super::accessible(&self.folders, user, root_id).await?;

        let file = self
            .trash
            .get_trashed(id)
            .await?
            .filter(|file| file.root_id == root_id)
            .ok_or_else(|| AppError::ResourceMissing("File does not exist".into()))?;

        let files = self.trash.list_trashed_tree_ids(root_id, file.id).await?;
        let request_id = Uuid::new_v4();

        DeletionQueue::submit(
            handle,
            request_id,
            PendingDeletion::from_refs(FileTable::Trashed, &files),
            None,
        )
        .await
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
        super::accessible(&self.folders, user, root_id).await?;

        let Some(trashed_root) = self.trash.trashed_root(id).await? else {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        };

        if trashed_root != root_id {
            return Err(AppError::Unprocessable(
                "Files can only be restored to their original roots".into(),
            ));
        }

        self.validate_restore_parent(root_id, input.parent).await?;

        let quota = self.files.root_quota(root_id).await?;

        let used = if quota > 0 {
            self.files.root_quota_usage(root_id).await?
        } else {
            0
        };

        let mut tx = self.tx.begin().await?;

        let mut restored = self
            .trash
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

        for file in restored.iter_mut() {
            if file.id == id {
                file.parent = input.parent;
                break;
            }
        }

        self.files.insert_many(&mut *tx, &restored).await?;

        tx.commit().await?;

        Ok(())
    }

    /// Restore every trashed item in a root under `input.parent`.
    #[instrument(skip(self, user, target), fields(user_id = %user.id, root_id = %root_id), err(Debug))]
    pub async fn restore_all(
        &self,
        user: &User,
        root_id: Uuid,
        target: RestoreFile,
    ) -> Result<(), AppError> {
        super::accessible(&self.folders, user, root_id).await?;
        self.validate_restore_parent(root_id, target.parent).await?;

        let trash = self.trash.list_trash(root_id).await?;

        let root_ids: Vec<Uuid> = trash
            .iter()
            .filter(|entry| entry.query_root)
            .map(|entry| entry.id)
            .collect();

        if root_ids.is_empty() {
            return Ok(());
        }

        let quota = self.files.root_quota(root_id).await?;

        let used = if quota > 0 {
            self.files.root_quota_usage(root_id).await?
        } else {
            0
        };

        let mut tx = self.tx.begin().await?;
        let mut restored = Vec::new();

        for id in root_ids {
            let mut tree = self
                .trash
                .delete_trashed_tree(&mut *tx, root_id, id)
                .await?;

            for file in tree.iter_mut() {
                if file.id == id {
                    file.parent = target.parent;
                    break;
                }
            }

            restored.append(&mut tree);
        }

        let bytes: i64 = restored
            .iter()
            .filter(|row| row.kind == FileKind::File)
            .map(|row| row.size_bytes)
            .sum();

        if quota > 0 && used.saturating_add(bytes) > quota {
            return Err(AppError::QuotaExceeded);
        }

        self.files.insert_many(&mut *tx, &restored).await?;

        tx.commit().await?;

        Ok(())
    }

    async fn validate_restore_parent(
        &self,
        root_id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<(), AppError> {
        let Some(parent) = parent else {
            return Ok(());
        };

        let entry = self
            .files
            .get_live(parent)
            .await?
            .ok_or_else(|| AppError::Unprocessable("Target directory does not exist".into()))?;

        if entry.root_id != root_id || entry.kind != FileKind::Directory {
            return Err(AppError::Unprocessable("Target is not a directory".into()));
        }

        Ok(())
    }

    pub async fn get_trash_tree(&self, user: &User, root_id: Uuid) -> Result<FileTree, AppError> {
        super::accessible(&self.folders, user, root_id).await?;

        let files = self.trash.list_trash(root_id).await?;

        Ok(FileTree {
            id: root_id,
            name: String::new(),
            kind: FileKind::Directory,
            file_type: None,
            metadata: None,
            children: FileTree::from_entries(files),
        })
    }
}
