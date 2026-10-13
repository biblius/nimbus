use crate::{
    db::{Db, DbPool, files::FileTable},
    error::AppError,
    file::model::FileRef,
};
use chrono::Utc;
use sqlx::QueryBuilder;
use tracing::instrument;
use uuid::Uuid;

pub const SYSTEM: &str = "SYSTEM";

#[derive(Debug, Clone, Copy)]
pub struct QueuedDeletion {
    pub file_id: Uuid,
    pub origin: FileTable,
}

#[derive(Clone)]
pub struct DeletionRepository {
    pool: DbPool,
}

impl DeletionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Insert one queue row per file. Re-enqueue clears a prior error and rebinds the batch id.
    #[instrument(skip(self, files), err(Debug))]
    pub async fn enqueue(
        &self,
        id: Uuid,
        created_by: &str,
        origin: FileTable,
        files: &[FileRef],
    ) -> Result<(), AppError> {
        if files.is_empty() {
            return Ok(());
        }

        let now = Utc::now();

        for chunk in files.chunks(500) {
            let mut query: QueryBuilder<Db> = QueryBuilder::new(
                r#"
                INSERT INTO file_deletion_queue (
                    id,
                    file_id,
                    origin,
                    created_by,
                    created_at
                )
                "#,
            );

            query.push_values(chunk, |mut row, file| {
                row.push_bind(id)
                    .push_bind(file.id)
                    .push_bind(origin)
                    .push_bind(created_by)
                    .push_bind(now);
            });

            query.push(
                r#"
                ON CONFLICT (origin, file_id) DO UPDATE SET
                    id = EXCLUDED.id,
                    created_by = EXCLUDED.created_by,
                    created_at = EXCLUDED.created_at,
                    error = NULL
                "#,
            );

            query.build().execute(&self.pool).await?;
        }

        Ok(())
    }

    /// Pending rows for a request batch.
    #[instrument(skip(self), err(Debug))]
    pub async fn list_request(&self, id: Uuid) -> Result<Vec<QueuedDeletion>, AppError> {
        sqlx::query_as!(
            QueuedDeletion,
            r#"
            SELECT
                file_id,
                origin AS "origin: _"
            FROM file_deletion_queue
            WHERE error IS NULL
              AND id = $1
            ORDER BY file_id
            "#,
            id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    /// Pending rows queued by the expiry worker.
    #[instrument(skip(self), err(Debug))]
    pub async fn list_system(&self) -> Result<Vec<QueuedDeletion>, AppError> {
        sqlx::query_as!(
            QueuedDeletion,
            r#"
            SELECT
                file_id,
                origin AS "origin: _"
            FROM file_deletion_queue
            WHERE error IS NULL
              AND created_by = $1
            ORDER BY file_id
            "#,
            SYSTEM
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn set_error(
        &self,
        origin: FileTable,
        file_id: Uuid,
        error: String,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE file_deletion_queue
            SET error = $3
            WHERE origin = $1
              AND file_id = $2
            "#,
            origin as FileTable,
            file_id,
            error
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn unqueue(&self, origin: FileTable, file_id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            DELETE FROM file_deletion_queue
            WHERE origin = $1
              AND file_id = $2
            "#,
            origin as FileTable,
            file_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
