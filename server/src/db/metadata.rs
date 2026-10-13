use crate::{db::DbPool, error::AppError};
use sqlx::FromRow;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct FileMetadata {
    pub file_id: Uuid,
    pub preview: Option<String>,
}

#[derive(Clone)]
pub struct MetadataRepository {
    pool: DbPool,
}

impl MetadataRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Insert or update the preview hash for a file.
    #[instrument(skip(self), err(Debug))]
    pub async fn set_preview(&self, file_id: Uuid, preview: &str) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO file_metadata (file_id, preview)
            VALUES ($1, $2)
            ON CONFLICT (file_id) DO UPDATE SET
                preview = EXCLUDED.preview
            "#,
        )
        .bind(file_id)
        .bind(preview)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn get(&self, file_id: Uuid) -> Result<Option<FileMetadata>, AppError> {
        sqlx::query_as::<_, FileMetadata>(
            r#"
            SELECT file_id, preview
            FROM file_metadata
            WHERE file_id = $1
            "#,
        )
        .bind(file_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }
}
