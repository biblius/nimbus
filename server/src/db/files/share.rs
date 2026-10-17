use crate::{
    db::DbPool,
    error::AppError,
    file::{
        format::FileType,
        model::{FileEntry, FileKind, FileMetadata, FileView},
    },
};
use chrono::Utc;
use sqlx::{QueryBuilder, types::Json};
use tracing::instrument;
use uuid::Uuid;

/// Queries over live files with `shared = true`.
///
/// Every read in this repository filters on the share flag so it only ever
/// touches publicly shared files. Share/unshare mutations also live here.
#[derive(Clone)]
pub struct ShareRepository {
    pool: DbPool,
}

impl ShareRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// A live file that is currently publicly shared.
    #[instrument(skip(self), err(Debug))]
    pub async fn get_shared(&self, id: Uuid) -> Result<Option<FileEntry>, AppError> {
        sqlx::query_as!(
            FileEntry,
            r#"
            SELECT
                id,
                root_id,
                created_by AS "created_by?",
                "type" AS "kind: FileKind",
                name,
                parent,
                format AS "file_type: FileType",
                hash,
                size_bytes,
                shared,
                metadata AS "metadata: Json<FileMetadata>",
                created_at,
                updated_at
            FROM files
            WHERE id = $1
              AND shared
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    /// Shared children of a shared directory.
    #[instrument(skip(self), err(Debug))]
    pub async fn list_shared_children(&self, parent: Uuid) -> Result<Vec<FileView>, AppError> {
        let mut query = QueryBuilder::new(
            r#"
            SELECT
                f.id,
                f.root_id,
                f.created_by,
                f."type" AS kind,
                f.name,
                f.parent,
                f.format AS file_type,
                f.hash,
                f.size_bytes,
                f.shared,
                f.metadata,
                f.created_at,
                f.updated_at
            FROM files f
            WHERE f.shared
              AND f.parent =
            "#,
        );

        query.push_bind(parent);
        query.push(
            r#" ORDER BY CASE WHEN f."type" = 'd' THEN 0 ELSE 1 END, f.name, f.created_at, f.id"#,
        );

        let rows = query
            .build_query_as::<FileEntry>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(rows.into_iter().map(FileView::from).collect())
    }

    /// Set `shared` on `id` and every live descendant in `root_id`.
    #[instrument(skip(self), fields(root_id = %root_id, file_id = %id, shared), err(Debug))]
    pub async fn set_shared_subtree(
        &self,
        root_id: Uuid,
        id: Uuid,
        shared: bool,
    ) -> Result<(), AppError> {
        let now = Utc::now();

        let result = sqlx::query!(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id
                FROM files
                WHERE id = $1
                  AND root_id = $2
                UNION ALL
                SELECT child.id
                FROM files child
                JOIN tree t ON child.parent = t.id
            )
            UPDATE files
            SET
                shared = $3,
                updated_at = $4
            WHERE id IN (SELECT id FROM tree)
            "#,
            id,
            root_id,
            shared,
            now
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        Ok(())
    }
}
