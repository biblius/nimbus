use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::{
        model::{FileEntry, FileView, SharingMode},
        tree::FileEntryTree,
    },
};
use chrono::{DateTime, Utc};
use sqlx::QueryBuilder;
use tracing::instrument;
use uuid::Uuid;

/// Queries over live files that currently have a public share.
///
/// Every read in this repository filters on sharing mode (and expiry) so it
/// only ever touches publicly shared, non-expired files. Share/unshare
/// mutations also live here.
#[derive(Clone)]
pub struct ShareRepository {
    pool: DbPool,
}

impl ShareRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// A live file that is currently publicly shared and not expired.
    #[instrument(skip(self), err(Debug))]
    pub async fn get_shared(&self, id: Uuid) -> Result<Option<FileEntry>, AppError> {
        let now = Utc::now();

        sqlx::query_as::<_, FileEntry>(
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
                f.sharing_mode,
                f.share_expires_at,
                m.preview_64,
                m.preview_256,
                f.created_at,
                f.updated_at
            FROM files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE f.id = $1
              AND f.sharing_mode IS NOT NULL
              AND (f.share_expires_at IS NULL OR f.share_expires_at > $2)
            "#,
        )
        .bind(id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    /// Shared descendants under `parent` (non-expired only), for zip downloads.
    #[instrument(skip(self), err(Debug))]
    pub async fn list_shared_descendants(
        &self,
        root_id: Uuid,
        parent: Uuid,
    ) -> Result<Vec<FileEntryTree>, AppError> {
        let now = Utc::now();

        let mut query = QueryBuilder::<Db>::new(
            r#"
            WITH RECURSIVE tree AS (
                SELECT
                    f.id,
                    f.root_id,
                    f."type" AS kind,
                    f.name,
                    f.parent,
                    f.format AS file_type,
                    f.hash,
                    f.size_bytes,
                    f.created_at,
                    f.updated_at,
                    TRUE AS query_root
                FROM files f
                WHERE f.root_id =
            "#,
        );

        query.push_bind(root_id);
        query.push(" AND f.parent = ").push_bind(parent);
        query.push(
            r#"
              AND f.sharing_mode IS NOT NULL
              AND (f.share_expires_at IS NULL OR f.share_expires_at >
            "#,
        );
        query.push_bind(now);
        query.push(
            r#"
              )
            UNION
            SELECT
                child.id,
                child.root_id,
                child."type",
                child.name,
                child.parent,
                child.format,
                child.hash,
                child.size_bytes,
                child.created_at,
                child.updated_at,
                FALSE AS query_root
            FROM files child
            JOIN tree t ON child.parent = t.id
            WHERE child.sharing_mode IS NOT NULL
              AND (child.share_expires_at IS NULL OR child.share_expires_at >
            "#,
        );
        query.push_bind(now);
        query.push(format!(
            r#"
              )
            )
            SELECT
                t.id,
                t.root_id,
                t.kind,
                t.name,
                t.parent,
                t.file_type,
                t.hash,
                t.size_bytes,
                m.preview_64,
                m.preview_256,
                t.created_at,
                t.updated_at,
                t.query_root
            FROM tree t
            LEFT JOIN file_metadata m ON m.file_id = t.id
            ORDER BY CASE WHEN t.kind = 'd' THEN 0 ELSE 1 END, t.name, t.created_at, t.id
            "#
        ));

        query
            .build_query_as::<FileEntryTree>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)
    }

    /// Shared children of a shared directory (non-expired only).
    #[instrument(skip(self), err(Debug))]
    pub async fn list_shared_children(&self, parent: Uuid) -> Result<Vec<FileView>, AppError> {
        let now = Utc::now();

        let mut query = QueryBuilder::new(format!(
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
                f.sharing_mode,
                f.share_expires_at,
                m.preview_64,
                m.preview_256,
                f.created_at,
                f.updated_at
            FROM files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE f.sharing_mode IS NOT NULL
              AND (f.share_expires_at IS NULL OR f.share_expires_at > 
            "#
        ));

        query.push_bind(now);
        query.push(") AND f.parent = ");
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

    /// Set share mode and expiry on `id` and every live descendant in `root_id`.
    ///
    /// Pass `sharing_mode: None` (and typically `share_expires_at: None`) to clear
    /// the public share on the whole subtree.
    #[instrument(
        skip(self),
        fields(root_id = %root_id, file_id = %id, ?sharing_mode, ?share_expires_at),
        err(Debug)
    )]
    pub async fn set_sharing_subtree(
        &self,
        root_id: Uuid,
        id: Uuid,
        sharing_mode: Option<SharingMode>,
        share_expires_at: Option<DateTime<Utc>>,
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
                sharing_mode = $3,
                share_expires_at = $4,
                updated_at = $5
            WHERE id IN (SELECT id FROM tree)
            "#,
            id,
            root_id,
            sharing_mode as Option<SharingMode>,
            share_expires_at,
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
