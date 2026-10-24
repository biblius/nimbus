use crate::{
    db::{Repository, files::FileTable},
    error::AppError,
    file::{
        model::{Cleanup, FileEntry, FileKind, FileRef},
        storage::Storage,
    },
};
use chrono::{Duration, Utc};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    time::Duration as StdDuration,
};
use tokio::{
    sync::mpsc,
    time::{MissedTickBehavior, interval},
};
use tracing::instrument;
use uuid::Uuid;

/// One file the worker finished attempting.
#[derive(Debug, Clone, Serialize)]
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
    pub root_id: Uuid,
    pub origin: FileTable,
}

impl PendingDeletion {
    pub fn from_refs(origin: FileTable, files: &[FileRef]) -> Vec<Self> {
        files
            .iter()
            .map(|file| Self {
                file_id: file.id,
                root_id: file.root_id,
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

        tracing::debug!("running expired trash sweep for {} items", expired.len());

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

        // Group by root so deletes hit `(root_id, id)` indexes.
        let mut live_by_root: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        let mut trash_by_root: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

        for entry in files.iter() {
            match entry.origin {
                FileTable::Live => live_by_root
                    .entry(entry.root_id)
                    .or_default()
                    .push(entry.file_id),
                FileTable::Trashed => trash_by_root
                    .entry(entry.root_id)
                    .or_default()
                    .push(entry.file_id),
            }
        }

        let mut live = Vec::new();
        
        for (root_id, ids) in &live_by_root {
            live.extend(self.repository.files.get_live_many(*root_id, ids).await?);
        }

        let mut trashed = Vec::new();

        for (root_id, ids) in &trash_by_root {
            trashed.extend(
                self.repository
                    .trash
                    .get_trashed_many(*root_id, ids)
                    .await?,
            );
        }

        let cap = live.len() + trashed.len();

        let by_id: HashMap<Uuid, FileEntry> = live.into_iter().chain(trashed.into_iter()).fold(
            HashMap::with_capacity(cap),
            |mut acc, file| {
                acc.insert(file.id, file);
                acc
            },
        );

        let mut deleted = HashSet::new();

        for (root_id, ids) in &live_by_root {
            deleted.extend(self.repository.files.delete_many(*root_id, ids).await?);
        }

        for (root_id, ids) in &trash_by_root {
            deleted.extend(self.repository.trash.delete_many(*root_id, ids).await?);
        }

        let mut content_hashes = HashSet::new();
        let mut preview_hashes = HashSet::new();

        for entry in files.iter() {
            let Some(file) = by_id.get(&entry.file_id) else {
                tracing::debug!(?entry, "file was not deleted");
                failed = true;
                continue;
            };

            // If DB entry was successfully deleted, clients can consider this a success
            // since it is no longer accessible through the API.
            let success = deleted.contains(&file.id);

            if !success {
                tracing::error!(file=%file.id, "failed to delete file row");
                failed = true;
            }

            if let Some(request_id) = request_id {
                self.send_result(DeletionResult {
                    request_id,
                    file_id: file.id,
                    name: file.name.clone(),
                    success,
                })
                .await;
            }

            if !success || file.kind == FileKind::Directory {
                continue;
            }

            let Some(ref hash) = file.hash else {
                tracing::error!(file=%file.id, "file has no hash");
                failed = true;
                continue;
            };

            content_hashes.insert(hash.as_str());

            if let Some(ref preview) = file.metadata.preview_64 {
                preview_hashes.insert(preview.as_str());
            }

            if let Some(ref preview) = file.metadata.preview_256 {
                preview_hashes.insert(preview.as_str());
            }

            if let Some(ref preview) = file.metadata.preview_blob {
                preview_hashes.insert(preview.as_str());
            }
        }

        // Counts are after the row deletes, so this batch is already excluded.
        let content_hashes: Vec<_> = content_hashes.into_iter().collect();
        let preview_hashes: Vec<_> = preview_hashes.into_iter().collect();

        let hash_counts = match self.repository.files.count_hashes(&content_hashes).await {
            Ok(counts) => counts,
            Err(e) => {
                tracing::error!(error=%e, "cannot count remaining hashes, will not delete BLOBs");
                failed = true;
                HashMap::new()
            }
        };

        for hash in content_hashes {
            let remaining = hash_counts.get(hash).copied().unwrap_or(0);

            if remaining > 0 {
                tracing::debug!(%remaining, %hash, "file hash still in use");
                continue;
            }

            match self.storage.remove(hash).await {
                Ok(()) => tracing::info!(%hash, "deleted BLOB"),
                Err(e) => {
                    tracing::error!(error=%e, %hash, "failed to delete file BLOB");
                    failed = true;
                }
            }
        }

        let preview_counts = match self.repository.files.count_previews(&preview_hashes).await {
            Ok(counts) => counts,
            Err(e) => {
                tracing::error!(error=%e, "cannot count remaining previews, will not delete preview BLOBs");
                failed = true;
                HashMap::new()
            }
        };

        for preview in preview_hashes {
            let remaining = preview_counts.get(preview).copied().unwrap_or(0);

            if remaining > 0 {
                tracing::debug!(%remaining, %preview, "preview hash still in use");
                continue;
            }

            match self.storage.remove_preview(preview).await {
                Ok(()) => tracing::info!(%preview, "deleted preview BLOB"),
                Err(e) => {
                    tracing::error!(error=%e, %preview, "failed to delete preview BLOB");
                    failed = true;
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
