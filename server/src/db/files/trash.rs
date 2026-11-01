use crate::{
    db::{
        Db, DbPool,
        files::{FileTable, list_descendants_in},
    },
    error::AppError,
    file::{
        model::{FileEntry, FileRef},
        tree::FileEntryTree,
    },
};
use chrono::{DateTime, Utc};
use sqlx::{Executor, QueryBuilder};
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct TrashRepository {
    pool: DbPool,
}

impl TrashRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn get_trashed(
        &self,
        root_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FileEntry>, AppError> {
        self.get_trashed_with(&self.pool, root_id, id).await
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn get_trashed_with<'e, E>(
        &self,
        executor: E,
        root_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FileEntry>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
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
                NULL AS sharing_mode,
                NULL AS share_expires_at,
                m.preview_64,
                m.preview_256,
                m.preview_blob,
                f.created_at,
                f.updated_at
            FROM trashed_files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE f.root_id = $1 AND f.id = $2
            "#,
        )
        .bind(root_id)
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(AppError::from)
    }

    /// Trashed rows for every id in `ids` within `root_id`. Missing ids are omitted.
    #[instrument(skip(self, ids), err(Debug))]
    pub async fn get_trashed_many(
        &self,
        root_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<FileEntry>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query = QueryBuilder::<Db>::new(
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
                NULL AS sharing_mode,
                NULL AS share_expires_at,
                m.preview_64,
                m.preview_256,
                m.preview_blob,
                f.created_at,
                f.updated_at
            FROM trashed_files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE f.root_id =
            "#,
        );

        query.push_bind(root_id);
        query.push(" AND f.id IN (");

        {
            let mut selected = query.separated(", ");
            for id in ids {
                selected.push_bind(*id);
            }
        }

        query.push(")");

        query
            .build_query_as::<FileEntry>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)
    }

    /// Trash for a root. Directly trashed rows have a null parent, so they are the roots.
    pub async fn list_trash(&self, root_id: Uuid) -> Result<Vec<FileEntryTree>, AppError> {
        list_descendants_in(&self.pool, FileTable::Trashed, root_id, None).await
    }

    /// Move `ids` and every descendant out of `files` and into `trashed_files`.
    ///
    /// A moved row whose parent was not moved becomes a trash root.
    /// Preview metadata stays in `file_metadata` (same file id).
    ///
    /// Done with `INSERT…SELECT` + CTE `DELETE` so descendants are never bound
    /// as SQL variables (only the seed `ids` are).
    #[instrument(skip(self), err(Debug))]
    pub async fn trash(
        &self,
        root_id: Uuid,
        trashed_by: Uuid,
        ids: &[Uuid],
    ) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        let now = Utc::now();

        let mut insert = QueryBuilder::<Db>::new(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id
                FROM files
                WHERE root_id =
            "#,
        );
        insert.push_bind(root_id);
        insert.push(" AND id IN (");
        {
            let mut selected = insert.separated(", ");
            for id in ids {
                selected.push_bind(*id);
            }
        }
        // UNION (not ALL): selected ids can also appear as descendants.
        insert.push(
            r#"
                )
                UNION
                SELECT f.id
                FROM files f
                JOIN tree t ON f.parent = t.id
            )
            INSERT INTO trashed_files (
                id,
                root_id,
                created_by,
                trashed_by,
                parent,
                "type",
                name,
                format,
                hash,
                size_bytes,
                created_at,
                updated_at,
                trashed_at
            )
            SELECT
                f.id,
                f.root_id,
                f.created_by,
            "#,
        );
        insert.push_bind(trashed_by);
        insert.push(
            r#"
                ,
                CASE
                    WHEN f.parent IN (SELECT id FROM tree) THEN f.parent
                    ELSE NULL
                END,
                f."type",
                f.name,
                f.format,
                f.hash,
                f.size_bytes,
                f.created_at,
            "#,
        );
        insert.push_bind(now);
        insert.push(",");
        insert.push_bind(now);
        insert.push(
            r#"
            FROM tree t
            JOIN files f ON f.id = t.id
            "#,
        );

        let inserted = insert.build().execute(&mut *tx).await?.rows_affected();
        if inserted == 0 {
            tracing::warn!("trash did not trash any rows");
            return Ok(());
        }

        let mut delete = QueryBuilder::<Db>::new(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id
                FROM files
                WHERE root_id =
            "#,
        );
        delete.push_bind(root_id);
        delete.push(" AND id IN (");
        {
            let mut selected = delete.separated(", ");
            for id in ids {
                selected.push_bind(*id);
            }
        }
        delete.push(
            r#"
                )
                UNION
                SELECT f.id
                FROM files f
                JOIN tree t ON f.parent = t.id
            )
            DELETE FROM files
            WHERE root_id =
            "#,
        );
        delete.push_bind(root_id);
        delete.push(" AND id IN (SELECT id FROM tree)");
        delete.build().execute(&mut *tx).await?;

        tx.commit().await?;

        Ok(())
    }

    /// Total `size_bytes` of files in the trashed subtree rooted at `id`.
    ///
    /// `None` when `id` is not a trashed row in `root_id`.
    #[instrument(skip(self, conn), err(Debug))]
    pub async fn trashed_tree_bytes(
        &self,
        conn: &mut <Db as sqlx::Database>::Connection,
        root_id: Uuid,
        id: Uuid,
    ) -> Result<Option<i64>, AppError> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM trashed_files WHERE id = $1 AND root_id = $2
            )
            "#,
        )
        .bind(id)
        .bind(root_id)
        .fetch_one(&mut *conn)
        .await?;

        if !exists {
            return Ok(None);
        }

        let bytes = sqlx::query_scalar::<_, i64>(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id
                FROM trashed_files
                WHERE id = $1
                  AND root_id = $2
                UNION ALL
                SELECT child.id
                FROM trashed_files child
                JOIN tree t ON child.parent = t.id
            )
            SELECT CAST(COALESCE(SUM(f.size_bytes), 0) AS BIGINT)
            FROM tree t
            JOIN trashed_files f ON f.id = t.id
            WHERE f."type" = 'f'
            "#,
        )
        .bind(id)
        .bind(root_id)
        .fetch_one(&mut *conn)
        .await?;

        Ok(Some(bytes))
    }

    /// Total `size_bytes` of every trashed file in `root_id`.
    #[instrument(skip(self, conn), err(Debug))]
    pub async fn trashed_root_bytes(
        &self,
        conn: &mut <Db as sqlx::Database>::Connection,
        root_id: Uuid,
    ) -> Result<i64, AppError> {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT CAST(COALESCE(SUM(size_bytes), 0) AS BIGINT)
            FROM trashed_files
            WHERE root_id = $1
              AND "type" = 'f'
            "#,
        )
        .bind(root_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(AppError::from)
    }

    /// Move `id` and its descendants from `trashed_files` into `files`.
    ///
    /// The restored root is reparented to `parent` (trash-root semantics for that
    /// row only). Descendants keep their in-trash parents. `file_metadata` is
    /// left alone (same ids).
    ///
    /// Returns how many rows were moved.
    #[instrument(skip(self, conn), err(Debug))]
    pub async fn restore_tree(
        &self,
        conn: &mut <Db as sqlx::Database>::Connection,
        root_id: Uuid,
        id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<u64, AppError> {
        let inserted = sqlx::query(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id
                FROM trashed_files
                WHERE id = $1
                  AND root_id = $2
                UNION ALL
                SELECT child.id
                FROM trashed_files child
                JOIN tree t ON child.parent = t.id
            )
            INSERT INTO files (
                id,
                root_id,
                created_by,
                "type",
                name,
                parent,
                format,
                hash,
                size_bytes,
                sharing_mode,
                share_expires_at,
                created_at,
                updated_at
            )
            SELECT
                f.id,
                f.root_id,
                f.created_by,
                f."type",
                f.name,
                CASE WHEN f.id = $1 THEN $3 ELSE f.parent END,
                f.format,
                f.hash,
                f.size_bytes,
                NULL,
                NULL,
                f.created_at,
                f.updated_at
            FROM tree t
            JOIN trashed_files f ON f.id = t.id
            "#,
        )
        .bind(id)
        .bind(root_id)
        .bind(parent)
        .execute(&mut *conn)
        .await?
        .rows_affected();

        if inserted == 0 {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        sqlx::query(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id
                FROM trashed_files
                WHERE id = $1
                  AND root_id = $2
                UNION ALL
                SELECT child.id
                FROM trashed_files child
                JOIN tree t ON child.parent = t.id
            )
            DELETE FROM trashed_files
            WHERE root_id = $2
              AND id IN (SELECT id FROM tree)
            "#,
        )
        .bind(id)
        .bind(root_id)
        .execute(&mut *conn)
        .await?;

        Ok(inserted)
    }

    /// Move every trashed row in `root_id` into `files`.
    ///
    /// Former trash roots (`parent IS NULL`) are reparented to `parent`.
    /// `file_metadata` is left alone.
    ///
    /// Returns `(rows_moved, sample_id)` for activity, or `None` when trash is empty.
    #[instrument(skip(self, conn), err(Debug))]
    pub async fn restore_all(
        &self,
        conn: &mut <Db as sqlx::Database>::Connection,
        root_id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<Option<(u64, Uuid)>, AppError> {
        let sample_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM trashed_files WHERE root_id = $1 LIMIT 1",
        )
        .bind(root_id)
        .fetch_optional(&mut *conn)
        .await?;

        let Some(sample_id) = sample_id else {
            return Ok(None);
        };

        let inserted = sqlx::query(
            r#"
            INSERT INTO files (
                id,
                root_id,
                created_by,
                "type",
                name,
                parent,
                format,
                hash,
                size_bytes,
                sharing_mode,
                share_expires_at,
                created_at,
                updated_at
            )
            SELECT
                f.id,
                f.root_id,
                f.created_by,
                f."type",
                f.name,
                CASE WHEN f.parent IS NULL THEN $2 ELSE f.parent END,
                f.format,
                f.hash,
                f.size_bytes,
                NULL,
                NULL,
                f.created_at,
                f.updated_at
            FROM trashed_files f
            WHERE f.root_id = $1
            "#,
        )
        .bind(root_id)
        .bind(parent)
        .execute(&mut *conn)
        .await?
        .rows_affected();

        sqlx::query("DELETE FROM trashed_files WHERE root_id = $1")
            .bind(root_id)
            .execute(&mut *conn)
            .await?;

        Ok(Some((inserted, sample_id)))
    }

    /// Delete trashed rows for `ids` in `root_id`. Returns the ids that were actually removed.
    ///
    /// Also removes matching `file_metadata` rows (hard delete).
    #[instrument(skip(self, ids), err(Debug))]
    pub async fn delete_many(&self, root_id: Uuid, ids: &[Uuid]) -> Result<Vec<Uuid>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin().await?;

        let mut query = QueryBuilder::<Db>::new("DELETE FROM trashed_files WHERE root_id = ");
        query.push_bind(root_id);
        query.push(" AND id IN (");
        {
            let mut selected = query.separated(", ");
            for id in ids {
                selected.push_bind(*id);
            }
        }
        query.push(") RETURNING id");

        let deleted = query
            .build_query_scalar::<Uuid>()
            .fetch_all(&mut *tx)
            .await
            .map_err(AppError::from)?;

        if !deleted.is_empty() {
            let mut meta = QueryBuilder::<Db>::new("DELETE FROM file_metadata WHERE file_id IN (");
            {
                let mut selected = meta.separated(", ");
                for id in &deleted {
                    selected.push_bind(*id);
                }
            }
            meta.push(")");
            meta.build().execute(&mut *tx).await?;
        }

        tx.commit().await?;

        Ok(deleted)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_trashed_ids(&self, root_id: Uuid) -> Result<Vec<FileRef>, AppError> {
        sqlx::query_as::<_, FileRef>(
            r#"
            SELECT
                id,
                root_id
            FROM trashed_files
            WHERE root_id = $1
            "#,
        )
        .bind(root_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    /// A trashed row and every descendant still attached below it.
    #[instrument(skip(self), err(Debug))]
    pub async fn list_trashed_tree_ids(
        &self,
        root_id: Uuid,
        id: Uuid,
    ) -> Result<Vec<FileRef>, AppError> {
        sqlx::query_as::<_, FileRef>(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id, root_id
                FROM trashed_files
                WHERE id = $1
                  AND root_id = $2

                UNION ALL

                SELECT child.id, child.root_id
                FROM trashed_files child
                JOIN tree parent ON child.parent = parent.id
            )
            SELECT
                id,
                root_id
            FROM tree
            "#,
        )
        .bind(id)
        .bind(root_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    /// Trashed rows with `trashed_at` before `cutoff` (past retention).
    #[instrument(skip(self), err(Debug))]
    pub async fn list_expired_trash(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<FileRef>, AppError> {
        sqlx::query_as::<_, FileRef>(
            r#"
            SELECT
                id,
                root_id
            FROM trashed_files
            WHERE trashed_at < $1
            "#,
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }
}
