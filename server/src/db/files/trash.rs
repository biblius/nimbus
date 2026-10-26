use std::collections::HashSet;

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
    /// Rows are selected before delete: SQLite applies `ON DELETE SET NULL`
    /// immediately while deleting, which would wipe parent links in `RETURNING`.
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

        let mut select = QueryBuilder::<Db>::new(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id
                FROM files
                WHERE root_id =
            "#,
        );

        select.push_bind(root_id);
        select.push(" AND id IN (");

        {
            let mut selected = select.separated(", ");
            for id in ids {
                selected.push_bind(*id);
            }
        }

        // UNION (not ALL): selected ids can also appear as descendants.
        select.push(
            r#"
                )
                UNION
                SELECT f.id
                FROM files f
                JOIN tree t ON f.parent = t.id
            )
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
            FROM tree t
            JOIN files f ON f.id = t.id
            LEFT JOIN file_metadata m ON m.file_id = f.id
            "#,
        );

        let trashed = select
            .build_query_as::<FileEntry>()
            .fetch_all(&mut *tx)
            .await?;

        if trashed.is_empty() {
            tracing::warn!("trash did not trash any rows");
            return Ok(());
        }

        let removed: HashSet<Uuid> = trashed.iter().map(|file| file.id).collect();

        let mut delete = QueryBuilder::<Db>::new("DELETE FROM files WHERE root_id = ");
        delete.push_bind(root_id);
        delete.push(" AND id IN (");
        {
            let mut selected = delete.separated(", ");
            for id in &removed {
                selected.push_bind(*id);
            }
        }
        delete.push(")");
        delete.build().execute(&mut *tx).await?;

        let now = Utc::now();

        let mut insert = QueryBuilder::<Db>::new(
            r#"
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
            "#,
        );

        insert.push_values(&trashed, |mut row, file| {
            // Entries whose parents are not in the removed set
            // become new roots
            let parent = file
                .parent
                .and_then(|parent| removed.contains(&parent).then_some(parent));

            row.push_bind(file.id)
                .push_bind(file.root_id)
                .push_bind(file.created_by)
                .push_bind(trashed_by)
                .push_bind(parent)
                .push_bind(file.kind)
                .push_bind(&file.name)
                .push_bind(&file.file_type)
                .push_bind(&file.hash)
                .push_bind(file.size_bytes)
                .push_bind(file.created_at)
                .push_bind(now)
                .push_bind(now);
        });

        insert.build().execute(&mut *tx).await?;

        tx.commit().await?;

        Ok(())
    }

    /// Delete `id` and its descendants from `trashed_files`. Returns the removed rows.
    ///
    /// Does not touch `file_metadata` because the same IDs are reused on restore.
    ///
    /// Selects before delete so parent links survive SQLite's immediate
    /// `ON DELETE SET NULL` during the multi-row delete.
    #[instrument(skip(self, conn), err(Debug))]
    pub async fn delete_tree(
        &self,
        conn: &mut <Db as sqlx::Database>::Connection,
        root_id: Uuid,
        id: Uuid,
    ) -> Result<Vec<FileEntry>, AppError> {
        let restored = sqlx::query_as::<_, FileEntry>(
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
            FROM tree t
            JOIN trashed_files f ON f.id = t.id
            LEFT JOIN file_metadata m ON m.file_id = f.id
            "#,
        )
        .bind(id)
        .bind(root_id)
        .fetch_all(&mut *conn)
        .await?;

        if restored.iter().all(|row| row.id != id) {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        let mut delete = QueryBuilder::<Db>::new("DELETE FROM trashed_files WHERE root_id = ");
        delete.push_bind(root_id);
        delete.push(" AND id IN (");
        {
            let mut selected = delete.separated(", ");
            for row in &restored {
                selected.push_bind(row.id);
            }
        }
        delete.push(")");
        delete.build().execute(&mut *conn).await?;

        Ok(restored)
    }

    /// Delete every trashed row in `root_id`. Returns the removed rows.
    ///
    /// Does not touch `file_metadata` because the same IDs are reused on restore.
    ///
    /// Selects before delete so parent links survive SQLite's immediate
    /// `ON DELETE SET NULL` during the multi-row delete.
    #[instrument(skip(self, conn), err(Debug))]
    pub async fn delete_all(
        &self,
        conn: &mut <Db as sqlx::Database>::Connection,
        root_id: Uuid,
    ) -> Result<Vec<FileEntry>, AppError> {
        let restored = sqlx::query_as::<_, FileEntry>(
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
            WHERE f.root_id = $1
            "#,
        )
        .bind(root_id)
        .fetch_all(&mut *conn)
        .await?;

        if !restored.is_empty() {
            sqlx::query("DELETE FROM trashed_files WHERE root_id = $1")
                .bind(root_id)
                .execute(&mut *conn)
                .await?;
        }

        Ok(restored)
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
