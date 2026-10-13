use crate::db::{
    deletion::{DeletionRepository, SYSTEM},
    files::{FileRepository, FileTable},
};
use chrono::{Duration, Utc};
use std::time::Duration as StdDuration;
use tokio::time::{MissedTickBehavior, interval};
use uuid::Uuid;

pub async fn start(
    files: FileRepository,
    deletions: DeletionRepository,
    retention_days: u16,
) {
    let mut tick = interval(StdDuration::from_secs(60));

    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tick.tick().await;

        let cutoff = Utc::now() - Duration::days(retention_days as i64);

        let expired = match files.list_expired_trash(cutoff).await {
            Ok(files) => files,
            Err(error) => {
                tracing::error!(%error, "expired trash lookup failed");
                continue;
            }
        };

        if expired.is_empty() {
            continue;
        }

        let id = Uuid::new_v4();

        if let Err(error) = deletions
            .enqueue(id, SYSTEM, FileTable::Trashed, &expired)
            .await
        {
            tracing::error!(%error, "failed to queue expired trash for deletion");
            continue;
        }
    }
}
