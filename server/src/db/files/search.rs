use super::FileRepository;
use crate::{
    error::AppError,
    file::model::{FileEntry, FileView},
};
use tracing::instrument;
use uuid::Uuid;

impl FileRepository {
    /// Fuzzy / substring name search within a root, best match first.
    ///
    /// Postgres uses `pg_trgm` (`%` / `similarity`). SQLite uses case-insensitive
    /// `LIKE '%q%'` (substring) with prefix-biased ordering — no trigram extension.
    #[instrument(skip(self, q), fields(root_id = %root_id, q = %q), err(Debug))]
    pub async fn search_by_name(&self, root_id: Uuid, q: &str) -> Result<Vec<FileView>, AppError> {
        let q = q.trim();
        if q.is_empty() {
            return Ok(vec![]);
        }

        #[cfg(feature = "postgres")]
        {
            self.search_by_name_postgres(root_id, q).await
        }

        #[cfg(feature = "sqlite")]
        {
            self.search_by_name_sqlite(root_id, q).await
        }
    }

    /// Every directory in a root, keyed by id (for client-side path crumbs).
    #[instrument(skip(self), fields(root_id = %root_id), err(Debug))]
    pub async fn list_directories(
        &self,
        root_id: Uuid,
    ) -> Result<std::collections::HashMap<Uuid, FileEntry>, AppError> {
        let rows = sqlx::query_as::<_, FileEntry>(
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
                m.preview_blob,
                f.created_at,
                f.updated_at
            FROM files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE f.root_id = $1 AND f."type" = 'd'
            "#,
        )
        .bind(root_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(|row| (row.id, row)).collect())
    }

    #[cfg(feature = "postgres")]
    async fn search_by_name_postgres(
        &self,
        root_id: Uuid,
        q: &str,
    ) -> Result<Vec<FileView>, AppError> {
        // Escape `\`, `%`, and `_` for use inside a SQL `LIKE` pattern with `ESCAPE '\'`.
        let mut like = String::with_capacity(q.len());
        let mut prefix = String::with_capacity(q.len());

        like.push('%');

        for c in q.chars() {
            match c {
                '\\' | '%' | '_' => {
                    like.push('\\');
                    like.push(c);
                    prefix.push('\\');
                    prefix.push(c);
                }
                _ => {
                    like.push(c);
                    prefix.push(c);
                }
            }
        }

        like.push('%');
        prefix.push('%');

        // Explicit similarity floor (slightly below pg_trgm's default 0.3 `%`
        // threshold) so short single-edit typos still match; ILIKE covers
        // substring / as-you-type where trigram score alone is weak.
        let rows = sqlx::query_as::<_, FileEntry>(
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
                m.preview_blob,
                f.created_at,
                f.updated_at
            FROM files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE
                f.root_id = $1
                AND (
                    similarity(f.name, $2) > 0.25
                    OR f.name ILIKE $3 ESCAPE '\'
                )
            ORDER BY
                similarity(f.name, $2) DESC,
                CASE WHEN f.name ILIKE $4 ESCAPE '\' THEN 0 ELSE 1 END,
                length(f.name),
                f.name,
                f.id
            "#,
        )
        .bind(root_id)
        .bind(q)
        .bind(like)
        .bind(prefix)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(FileView::from).collect())
    }

    #[cfg(feature = "sqlite")]
    async fn search_by_name_sqlite(
        &self,
        root_id: Uuid,
        q: &str,
    ) -> Result<Vec<FileView>, AppError> {
        // Substring match (no typo tolerance). Prefix hits rank first, then shorter names.
        // Escape `\`, `%`, and `_` for use inside a SQL `LIKE` pattern with `ESCAPE '\'`.
        let mut like = String::with_capacity(q.len());
        let mut prefix = String::with_capacity(q.len());

        like.push('%');

        for c in q.chars() {
            match c {
                '\\' | '%' | '_' => {
                    like.push('\\');
                    like.push(c);
                    prefix.push('\\');
                    prefix.push(c);
                }
                _ => {
                    like.push(c);
                    prefix.push(c);
                }
            }
        }

        like.push('%');
        prefix.push('%');

        let rows = sqlx::query_as::<_, FileEntry>(
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
                m.preview_blob,
                f.created_at,
                f.updated_at
            FROM files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE
                f.root_id = $1
                AND f.name LIKE $2 ESCAPE '\' COLLATE NOCASE
            ORDER BY
                CASE WHEN f.name LIKE $3 ESCAPE '\' COLLATE NOCASE THEN 0 ELSE 1 END,
                length(f.name),
                f.name,
                f.id
            "#,
        )
        .bind(root_id)
        .bind(&like)
        .bind(&prefix)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(FileView::from).collect())
    }
}
