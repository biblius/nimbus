use crate::{
    activity::{Activity, ActivityData, ActivityTarget},
    db::Repository,
    error::AppError,
};
use tokio::sync::mpsc;
use tracing::instrument;
use uuid::Uuid;

pub type ActivityQueueHandle = mpsc::Sender<ProcessActivity>;

/// Background worker that persists [`Activity`] rows to `root_activity`.
pub struct ActivityQueue {
    requests: mpsc::Receiver<ProcessActivity>,
    repository: Repository,
}

pub enum ProcessActivity {
    /// Activites that can be directly constructed in memory and are
    /// not expensive to create in the service layer.
    Process(Activity),
    /// Activity that requires communicating with the DB to get file names.
    ProcessMove {
        root_id: Uuid,
        username: String,
        ids: Vec<Uuid>,
    },
    ProcessShare {
        root_id: Uuid,
        username: String,
        ids: Vec<Uuid>,
    },
    ProcessRestore {
        root_id: Uuid,
        username: String,
        ids: Vec<Uuid>,
    },
}

impl ActivityQueue {
    pub fn start(repository: Repository) -> ActivityQueueHandle {
        let (tx, rx) = mpsc::channel(256);
        ActivityQueue {
            requests: rx,
            repository,
        }
        ._start();
        tx
    }

    /// Enqueue a single activity row.
    pub async fn submit(
        handle: &ActivityQueueHandle,
        activity: ProcessActivity,
    ) -> Result<(), AppError> {
        handle.send(activity).await.map_err(|_| AppError::Internal)
    }

    /// Drain the channel and run inserts concurrently (bounded).
    #[instrument(skip(self))]
    fn _start(mut self) {
        tokio::spawn(async move {
            while let Some(activity) = self.requests.recv().await {
                if let Err(e) = self.process_activity(activity).await {
                    tracing::error!(error=%e, "could not process activity");
                }
            }

            tracing::info!("Activity request channel closed, shutting down worker");
        });
    }

    async fn process_activity(&self, activity: ProcessActivity) -> Result<(), AppError> {
        match activity {
            ProcessActivity::Process(activity) => self.repository.activity.insert(&activity).await,
            ProcessActivity::ProcessMove {
                root_id,
                username,
                ids,
            } => {
                let target = if ids.len() == 1 {
                    ActivityTarget::Single {
                        id: ids[0],
                        name: self.repository.files.get_file_name(root_id, ids[0]).await?,
                    }
                } else {
                    ActivityTarget::Multiple { count: ids.len() }
                };

                // The file is already moved
                let to = self
                    .repository
                    .files
                    .get_parent_name(root_id, ids[0])
                    .await?;

                self.repository
                    .activity
                    .insert(&Activity::new(
                        root_id,
                        username,
                        ActivityData::Move { target, to },
                    ))
                    .await
            }
            ProcessActivity::ProcessRestore {
                root_id,
                username,
                ids,
            } => {
                let target = if ids.len() == 1 {
                    ActivityTarget::Single {
                        id: ids[0],
                        name: self.repository.files.get_file_name(root_id, ids[0]).await?,
                    }
                } else {
                    ActivityTarget::Multiple { count: ids.len() }
                };

                let to = self
                    .repository
                    .files
                    .get_parent_name(root_id, ids[0])
                    .await?;

                self.repository
                    .activity
                    .insert(&Activity::new(
                        root_id,
                        username,
                        ActivityData::Restore { target, to },
                    ))
                    .await
            }
            ProcessActivity::ProcessShare {
                root_id,
                username,
                ids,
            } => {
                let target = if ids.len() == 1 {
                    let name = self.repository.files.get_file_name(root_id, ids[0]).await?;
                    ActivityTarget::Single { id: ids[0], name }
                } else {
                    ActivityTarget::Multiple { count: ids.len() }
                };

                self.repository
                    .activity
                    .insert(&Activity::new(
                        root_id,
                        username,
                        ActivityData::Share(target),
                    ))
                    .await
            }
        }
    }
}
