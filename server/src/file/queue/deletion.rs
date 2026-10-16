use crate::{
    db::{Repository, deletion::QueuedDeletion, files::FileTable},
    error::AppError,
    file::{
        model::{Cleanup, FileKind},
        storage::Storage,
    },
};
use serde::Serialize;
use std::{collections::HashMap, time::Duration};
use tokio::{
    sync::mpsc,
    time::{MissedTickBehavior, interval},
};
use tracing::instrument;
use utoipa::ToSchema;
use uuid::Uuid;

/// One file the worker finished attempting.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeletionResult {
    pub request_id: Uuid,
    pub file_id: Uuid,
    pub name: String,
    pub success: bool,
}

/// User submitted deletion request.
#[derive(Debug)]
pub struct DeletionRequest {
    /// The unique ID of the request.
    pub request_id: Uuid,

    /// Response handle.
    results: mpsc::Sender<DeletionResult>,

    cleanup: Option<Cleanup>,
}

pub type DeletionQueueHandle = mpsc::Sender<DeletionRequest>;

/// Handle to the deletion queue.
pub struct DeletionQueue {
    /// Direct deletion requests initiated by users.
    requests: mpsc::Receiver<DeletionRequest>,

    /// Registered listeners that are awaiting deletion results.
    /// Mostly used as a convenience.
    listeners: HashMap<Uuid, mpsc::Sender<DeletionResult>>,

    repository: Repository,

    storage: Storage,
}

impl DeletionQueue {
    pub fn start(
        repository: Repository,
        storage: Storage,
        sweep_interval: Duration,
    ) -> DeletionQueueHandle {
        let (tx, rx) = mpsc::channel(128);
        DeletionQueue::new(repository, storage, rx)._start(sweep_interval);
        tx
    }

    fn new(
        repository: Repository,
        storage: Storage,
        requests: mpsc::Receiver<DeletionRequest>,
    ) -> Self {
        Self {
            requests,
            listeners: HashMap::new(),
            repository,
            storage,
        }
    }

    /// Send a deletion request to the queue. The receiver closes when the batch is done.
    pub async fn submit(
        handle: DeletionQueueHandle,
        request_id: Uuid,
        cleanup: Option<Cleanup>,
    ) -> Result<mpsc::Receiver<DeletionResult>, AppError> {
        let (results, incoming) = mpsc::channel(64);

        handle
            .send(DeletionRequest {
                request_id,
                results,
                cleanup,
            })
            .await
            .map_err(|_| AppError::Internal)?;

        Ok(incoming)
    }

    #[instrument(skip(self))]
    fn _start(mut self, sweep_interval: Duration) {
        tokio::spawn(async move {
            let mut tick = interval(sweep_interval);
            tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    // Means requests have priority
                    biased;
                    request = self.requests.recv() => {
                        let Some(request) = request else {
                            tracing::info!("Deletion request channel closed, shutting down worker");
                            break;
                        };

                        let DeletionRequest { request_id, results, cleanup } = request;

                        self.listeners.insert(request_id, results);

                        if let Err(e) = self.delete_files(Some(request_id), cleanup).await {
                            tracing::error!(error=%e, "error processing request");
                        };

                        self.listeners.remove(&request_id);
                    },
                    _ = tick.tick() => {
                        tracing::debug!("running deletion queue sweep");
                        if let Err(e) = self.delete_files(None, None).await {
                            tracing::error!(error=%e, "error sweeping deletion queue");
                        }
                    },
                }
            }
        });
    }

    /// Handle a direct user request initiated from a service.
    /// If the server process is killed, worst thing that can happen is we have orphaned entries and cleanups won't run.
    #[instrument(skip(self), err(Debug))]
    async fn delete_files(
        &mut self,
        request_id: Option<Uuid>,
        cleanup: Option<Cleanup>,
    ) -> Result<(), AppError> {
        let pending = match request_id {
            Some(request_id) => self.repository.deletions.list_request(request_id).await?,
            None => self.repository.deletions.list_system().await?,
        };

        for entry in pending {
            // Check whether it exists

            let file = match entry.origin {
                FileTable::Live => self.repository.files.get_live(entry.file_id).await?,
                FileTable::Trashed => self.repository.trash.get_trashed(entry.file_id).await?,
            };

            let Some(file) = file else {
                tracing::debug!(?entry, "file is missing");

                self.fail_deletion(entry, "File not found in database".into())
                    .await;

                continue;
            };

            // Delete DB entry

            let result = match entry.origin {
                FileTable::Live => self.repository.files.delete_live(file.id).await,
                FileTable::Trashed => self.repository.trash.delete_trashed(file.id).await,
            };

            // If DB entry was successfully deleted, clients
            // can consider this a success since it is no longer
            // accessible through the API. Worst case if tx drops
            // is that clients get a success message, but the file
            // remains

            match result {
                Ok(()) => {
                    if let Some(request_id) = request_id {
                        self.send_result(DeletionResult {
                            request_id,
                            file_id: file.id,
                            name: file.name,
                            success: true,
                        })
                        .await;
                    }
                }
                Err(e) => {
                    tracing::error!(file=%file.id, error=%e, "failed to delete directory");

                    if let Some(request_id) = request_id {
                        self.send_result(DeletionResult {
                            request_id,
                            file_id: file.id,
                            name: file.name,
                            success: false,
                        })
                        .await;
                    }

                    continue;
                }
            };

            if file.kind == FileKind::Directory {
                self.unqueue(entry).await;
                continue;
            }

            // Should never happen
            let Some(hash) = file.hash else {
                tracing::error!(file=%file.id, "file has no hash");

                self.fail_deletion(entry, "File does not have hash".into())
                    .await;

                continue;
            };

            // Delete blob

            // Counts are after the row delete, so this file is already excluded.
            let remaining = match self.repository.files.count_hash(&hash).await {
                Ok(count) => count,
                Err(e) => {
                    tracing::error!(file=%file.id, error=%e, "cannot count remaining hash, will not delete BLOB");

                    self.fail_deletion(entry, format!("Could not count file hash: {e}"))
                        .await;

                    continue;
                }
            };

            if remaining > 0 {
                tracing::debug!(%remaining, file=%file.id, "file hash still in use");
            } else {
                match self.storage.remove(&hash).await {
                    Ok(()) => tracing::info!(file=%file.id, "deleted BLOB {hash}"),
                    Err(e) => {
                        tracing::error!(file=%file.id, error=%e, "failed to delete file BLOB");
                        self.fail_deletion(
                            entry,
                            format!("Could not remove file from BLOB storage: {e}"),
                        )
                        .await;
                    }
                }
            }

            // Delete preview

            if let Some(preview_hash) = file.preview {
                let remaining = match self.repository.files.count_preview(&preview_hash).await {
                    Ok(count) => count,
                    Err(e) => {
                        tracing::error!(file=%file.id, error=%e, "cannot count remaining preview, will not delete preview BLOB");

                        self.fail_deletion(entry, format!("Could not count preview hash: {e}"))
                            .await;

                        continue;
                    }
                };

                if remaining > 0 {
                    tracing::debug!(%remaining, file=%file.id, "preview hash still in use");
                } else {
                    match self.storage.remove_preview(&preview_hash).await {
                        Ok(()) => {
                            tracing::info!(
                                file=%file.id,
                                preview=%preview_hash,
                                "deleted preview BLOB"
                            );
                        }
                        Err(e) => {
                            tracing::error!(file=%file.id, error=%e, "failed to delete preview BLOB");

                            self.fail_deletion(
                                entry,
                                format!("Could not remove preview from BLOB storage: {e}"),
                            )
                            .await;

                            continue;
                        }
                    }
                }
            }

            self.unqueue(entry).await;
        }

        let Some(cleanup) = cleanup else {
            return Ok(());
        };

        // Leave root/user in place while any row for this batch is still pending.

        if let Some(request_id) = request_id {
            let pending = self.repository.deletions.list_request(request_id).await?;
            if !pending.is_empty() {
                tracing::warn!(%request_id, remaining = pending.len(), "skipping cleanup; deletions still pending");
                return Ok(());
            }
        }

        match cleanup {
            Cleanup::User { user_id, root_id } => {
                let mut tx = self.repository.start_tx().await?;

                // Root first: `roots.owner` references `users`.
                self.repository
                    .files
                    .delete_root_with(&mut *tx, root_id)
                    .await?;

                self.repository.users.delete_with(&mut *tx, user_id).await?;

                tx.commit().await?;

                Ok(())
            }
            Cleanup::Root { root_id } => {
                self.repository.files.delete_root(root_id).await?;

                Ok(())
            }
        }
    }

    async fn send_result(&self, result: DeletionResult) {
        if let Some(tx) = self.listeners.get(&result.request_id)
            && let Err(e) = tx.send(result).await
        {
            tracing::error!(error=%e , "error sending deletion result")
        };
    }

    /// Mark the deletion queue entry as failed. Worst case if this fails, the entry is
    /// requeued.
    async fn fail_deletion(&self, entry: QueuedDeletion, error: String) {
        if let Err(e) = self
            .repository
            .deletions
            .set_error(entry.origin, entry.file_id, error)
            .await
        {
            tracing::error!(file=%entry.file_id, error=%e, "unable to set deletion error, will be requeued");
        }
    }

    /// Remove the entry from the queue.
    async fn unqueue(&self, entry: QueuedDeletion) {
        if let Err(e) = self
            .repository
            .deletions
            .unqueue(entry.origin, entry.file_id)
            .await
        {
            tracing::error!(file=%entry.file_id, error=%e, "unable to unqueue, will be requeued");
        }
    }
}
