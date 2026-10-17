use crate::{
    db::{Repository, files::FileTable},
    error::AppError,
    file::{
        model::{Cleanup, FileKind, FileRef},
        storage::Storage,
    },
};
use chrono::{Duration, Utc};
use serde::Serialize;
use std::{collections::HashMap, time::Duration as StdDuration};
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

/// A file pending hard deletion in this worker batch.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingDeletion {
    pub file_id: Uuid,
    pub origin: FileTable,
}

impl PendingDeletion {
    pub fn from_refs(origin: FileTable, files: &[FileRef]) -> Vec<Self> {
        files
            .iter()
            .map(|file| Self {
                file_id: file.id,
                origin,
            })
            .collect()
    }
}

/// User submitted deletion request.
#[derive(Debug)]
pub struct DeletionRequest {
    /// The unique ID of the request.
    pub request_id: Uuid,

    /// Files to hard-delete for this batch.
    files: Vec<PendingDeletion>,

    /// Response handle.
    results: mpsc::Sender<DeletionResult>,

    cleanup: Option<Cleanup>,
}

pub type DeletionQueueHandle = mpsc::Sender<DeletionRequest>;

/// In-memory deletion worker.
pub struct DeletionQueue {
    /// Direct deletion requests initiated by users.
    requests: mpsc::Receiver<DeletionRequest>,

    /// Registered listeners that are awaiting deletion results.
    listeners: HashMap<Uuid, mpsc::Sender<DeletionResult>>,

    repository: Repository,

    storage: Storage,

    retention_days: u16,
}

impl DeletionQueue {
    pub fn start(
        repository: Repository,
        storage: Storage,
        sweep_interval: StdDuration,
        retention_days: u16,
    ) -> DeletionQueueHandle {
        let (tx, rx) = mpsc::channel(128);
        DeletionQueue::new(repository, storage, rx, retention_days)._start(sweep_interval);
        tx
    }

    fn new(
        repository: Repository,
        storage: Storage,
        requests: mpsc::Receiver<DeletionRequest>,
        retention_days: u16,
    ) -> Self {
        Self {
            requests,
            listeners: HashMap::new(),
            repository,
            storage,
            retention_days,
        }
    }

    /// Send a deletion request to the queue. The receiver closes when the batch is done.
    pub async fn submit(
        handle: DeletionQueueHandle,
        request_id: Uuid,
        files: Vec<PendingDeletion>,
        cleanup: Option<Cleanup>,
    ) -> Result<mpsc::Receiver<DeletionResult>, AppError> {
        let (results, incoming) = mpsc::channel(64);

        handle
            .send(DeletionRequest {
                request_id,
                files,
                results,
                cleanup,
            })
            .await
            .map_err(|_| AppError::Internal)?;

        Ok(incoming)
    }

    #[instrument(skip(self))]
    fn _start(mut self, sweep_interval: StdDuration) {
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

                        let DeletionRequest { request_id, files, results, cleanup } = request;

                        self.listeners.insert(request_id, results);

                        if let Err(e) = self.delete_files(Some(request_id), files, cleanup).await {
                            tracing::error!(error=%e, "error processing request");
                        };

                        self.listeners.remove(&request_id);
                    },
                    _ = tick.tick() => {
                        tracing::debug!("running expired trash sweep");
                        if let Err(e) = self.sweep_expired().await {
                            tracing::error!(error=%e, "error sweeping expired trash");
                        }
                    },
                }
            }
        });
    }

    /// Hard-delete trashed files past the retention window.
    #[instrument(skip(self), err(Debug))]
    async fn sweep_expired(&mut self) -> Result<(), AppError> {
        let cutoff = Utc::now() - Duration::days(self.retention_days as i64);
        let expired = self.repository.trash.list_expired_trash(cutoff).await?;

        if expired.is_empty() {
            return Ok(());
        }

        let files = PendingDeletion::from_refs(FileTable::Trashed, &expired);
        self.delete_files(None, files, None).await
    }

    /// Handle a direct user request or an expired-trash sweep.
    /// If the server process is killed mid-batch, remaining files stay until the next attempt.
    #[instrument(skip(self, files), err(Debug))]
    async fn delete_files(
        &mut self,
        request_id: Option<Uuid>,
        files: Vec<PendingDeletion>,
        cleanup: Option<Cleanup>,
    ) -> Result<(), AppError> {
        let mut failed = false;

        for entry in files {
            let file = match entry.origin {
                FileTable::Live => self.repository.files.get_live(entry.file_id).await?,
                FileTable::Trashed => self.repository.trash.get_trashed(entry.file_id).await?,
            };

            let Some(file) = file else {
                tracing::debug!(?entry, "file is missing");
                failed = true;
                continue;
            };

            let result = match entry.origin {
                FileTable::Live => self.repository.files.delete_live(file.id).await,
                FileTable::Trashed => self.repository.trash.delete_trashed(file.id).await,
            };

            // If DB entry was successfully deleted, clients can consider this a success
            // since it is no longer accessible through the API.

            match result {
                Ok(()) => {
                    if let Some(request_id) = request_id {
                        self.send_result(DeletionResult {
                            request_id,
                            file_id: file.id,
                            name: file.name.clone(),
                            success: true,
                        })
                        .await;
                    }
                }
                Err(e) => {
                    tracing::error!(file=%file.id, error=%e, "failed to delete file row");
                    failed = true;

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
                continue;
            }

            let Some(hash) = file.hash else {
                tracing::error!(file=%file.id, "file has no hash");
                failed = true;
                continue;
            };

            // Counts are after the row delete, so this file is already excluded.
            let remaining = match self.repository.files.count_hash(&hash).await {
                Ok(count) => count,
                Err(e) => {
                    tracing::error!(file=%file.id, error=%e, "cannot count remaining hash, will not delete BLOB");
                    failed = true;
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
                        failed = true;
                    }
                }
            }

            let preview_hashes = file
                .metadata
                .as_ref()
                .map(|meta| {
                    [
                        meta.preview_64.clone(),
                        meta.preview_256.clone(),
                    ]
                })
                .into_iter()
                .flatten()
                .flatten()
                .collect::<Vec<_>>();

            for preview_hash in preview_hashes {
                let remaining = match self.repository.files.count_preview(&preview_hash).await {
                    Ok(count) => count,
                    Err(e) => {
                        tracing::error!(file=%file.id, error=%e, "cannot count remaining preview, will not delete preview BLOB");
                        failed = true;
                        break;
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
                            failed = true;
                            break;
                        }
                    }
                }
            }
        }

        let Some(cleanup) = cleanup else {
            return Ok(());
        };

        if failed {
            tracing::warn!(?request_id, "skipping cleanup; some deletions failed");
            return Ok(());
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
}
