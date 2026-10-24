use crate::{
    activity::{ActivityQueue, ActivityQueueHandle, queue::ProcessActivity},
    db::{
        files::{FileRepository, FileTable, TrashRepository},
        tx::Transactor,
    },
    error::AppError,
    file::{
        access::{AccessMode, UserRootContext},
        model::{FileKind, RestoreFile},
        queue::{DeletionQueue, DeletionQueueHandle, DeletionResult, PendingDeletion},
        tree::FileTree,
    },
};
use tokio::sync::mpsc;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct TrashFileService {
    trash: TrashRepository,
    files: FileRepository,
    activity: ActivityQueueHandle,
    tx: Transactor,
}

impl TrashFileService {
    pub fn new(
        trash: TrashRepository,
        files: FileRepository,
        activity: ActivityQueueHandle,
        tx: Transactor,
    ) -> Self {
        Self {
            trash,
            files,
            activity,
            tx,
        }
    }

    /// Permanently delete one trashed item and its descendants.
    #[instrument(skip(self, ctx, handle), fields(user_id = %ctx.user.id, root_id = %ctx.root.id, file_id = %id), err(Debug))]
    pub async fn delete_trash(
        &self,
        ctx: &UserRootContext,
        id: Uuid,
        handle: DeletionQueueHandle,
    ) -> Result<mpsc::Receiver<DeletionResult>, AppError> {
        ctx.require(AccessMode::WRITE)?;

        let root_id = ctx.root.id;

        let file = self
            .trash
            .get_trashed(root_id, id)
            .await?
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
    #[instrument(skip(self, ctx, input), fields(user_id = %ctx.user.id, root_id = %ctx.root.id, file_id = %id), err(Debug))]
    pub async fn restore(
        &self,
        ctx: &UserRootContext,
        id: Uuid,
        input: RestoreFile,
    ) -> Result<(), AppError> {
        ctx.require(AccessMode::WRITE)?;

        let root_id = ctx.root.id;

        if self.trash.get_trashed(root_id, id).await?.is_none() {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        self.validate_restore_parent(root_id, input.parent).await?;

        let quota = self.files.root_quota(root_id).await?;

        let used = if quota > 0 {
            self.files.root_quota_usage(root_id).await?
        } else {
            0
        };

        let mut tx = self.tx.begin().await?;

        let mut restored = self.trash.delete_tree(&mut tx, root_id, id).await?;

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

        self.files.insert_many(&mut tx, &restored).await?;

        tx.commit().await?;

        if let Err(e) = ActivityQueue::submit(
            &self.activity,
            ProcessActivity::ProcessRestore {
                root_id: ctx.root.id,
                username: ctx.user.username.clone(),
                ids: vec![id],
            },
        )
        .await
        {
            tracing::error!(error = %e, "unable to submit activity to queue");
        }

        Ok(())
    }

    /// Restore every trashed item in a root under `input.parent`.
    #[instrument(skip(self, ctx, target), fields(user_id = %ctx.user.id, root_id = %ctx.root.id), err(Debug))]
    pub async fn restore_all(
        &self,
        ctx: &UserRootContext,
        target: RestoreFile,
    ) -> Result<(), AppError> {
        ctx.require(AccessMode::WRITE)?;

        let root_id = ctx.root.id;

        self.validate_restore_parent(root_id, target.parent).await?;

        let quota = self.files.root_quota(root_id).await?;

        let used = if quota > 0 {
            self.files.root_quota_usage(root_id).await?
        } else {
            0
        };

        let mut tx = self.tx.begin().await?;

        let mut restored = self.trash.delete_all(&mut tx, root_id).await?;

        if restored.is_empty() {
            tx.commit().await?;

            return Ok(());
        }

        let bytes: i64 = restored
            .iter()
            .filter(|row| row.kind == FileKind::File)
            .map(|row| row.size_bytes)
            .sum();

        if quota > 0 && used.saturating_add(bytes) > quota {
            return Err(AppError::QuotaExceeded);
        }

        // Trash roots have a null parent, point them at the restore target.
        for file in restored.iter_mut() {
            if file.parent.is_none() {
                file.parent = target.parent;
            }
        }

        self.files.insert_many(&mut tx, &restored).await?;

        tx.commit().await?;

        if let Err(e) = ActivityQueue::submit(
            &self.activity,
            ProcessActivity::ProcessRestore {
                root_id: ctx.root.id,
                username: ctx.user.username.clone(),
                ids: restored.into_iter().map(|file| file.id).collect(),
            },
        )
        .await
        {
            tracing::error!(error = %e, "unable to submit activity to queue");
        }

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
            .get_live(root_id, parent)
            .await?
            .ok_or_else(|| AppError::Unprocessable("Target directory does not exist".into()))?;

        if entry.kind != FileKind::Directory {
            return Err(AppError::Unprocessable("Target is not a directory".into()));
        }

        Ok(())
    }

    pub async fn get_trash_tree(&self, ctx: &UserRootContext) -> Result<FileTree, AppError> {
        let files = self.trash.list_trash(ctx.root.id).await?;

        Ok(FileTree {
            id: ctx.root.id,
            name: String::new(),
            kind: FileKind::Directory,
            file_type: None,
            metadata: Default::default(),
            children: FileTree::from_entries(files),
        })
    }
}
