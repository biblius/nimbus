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
    pub async fn get_trashed(&self, id: Uuid) -> Result<Option<FileEntry>, AppError> {
        self.get_trashed_with(&self.pool, id).await
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn get_trashed_with<'e, E>(
        &self,
        executor: E,
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
                f.created_at,
                f.updated_at
            FROM trashed_files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE f.id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
        .await
        .map_err(AppError::from)
    }

    /// Trashed rows for every id in `ids`. Missing ids are omitted.
    #[instrument(skip(self, ids), err(Debug))]
    pub async fn get_trashed_many(&self, ids: &[Uuid]) -> Result<Vec<FileEntry>, AppError> {
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
                f.created_at,
                f.updated_at
            FROM trashed_files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE f.id IN (
            "#,
        );

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

        let mut selected = delete.separated(", ");
        for id in ids {
            selected.push_bind(*id);
        }

        selected.push_unseparated(
            r#"
                )
                UNION ALL
                SELECT f.id
                FROM files f
                JOIN tree t ON f.parent = t.id
            )
            DELETE FROM files
            WHERE id IN (SELECT id FROM tree)
            RETURNING
                id,
                root_id,
                created_by,
                "type" AS kind,
                name,
                parent,
                format AS file_type,
                hash,
                size_bytes,
                sharing_mode,
                share_expires_at,
                NULL AS preview_64,
                NULL AS preview_256,
                created_at,
                updated_at
            "#,
        );

        let trashed = delete
            .build_query_as::<FileEntry>()
            .fetch_all(&mut *tx)
            .await?;

        if trashed.is_empty() {
            tracing::warn!("trash did not trash any rows");
            return Ok(());
        }

        let ids: HashSet<Uuid> = trashed.iter().map(|file| file.id).collect();

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
                .and_then(|parent| ids.contains(&parent).then_some(parent));

            row.push_bind(file.id)
                .push_bind(file.root_id)
                .push_bind(file.created_by)
                .push_bind(trashed_by)
                .push_bind(parent)
                .push_bind(file.kind)
                .push_bind(&file.name)
                .push_bind(file.file_type.clone())
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

    /// Root of one trashed row.
    #[instrument(skip(self), err(Debug))]
    pub async fn get_file_root_id(&self, id: Uuid) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar!(
            r#"
            SELECT root_id
            FROM trashed_files
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    /// Delete `id` and its descendants from `trashed_files`. Returns the removed rows.
    ///
    /// Does not touch `file_metadata` because the same IDs are reused on restore.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_tree<'e, E>(
        &self,
        executor: E,
        root_id: Uuid,
        id: Uuid,
    ) -> Result<Vec<FileEntry>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
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
            DELETE FROM trashed_files
            WHERE id IN (SELECT id FROM tree)
            RETURNING
                id,
                root_id,
                created_by,
                "type" AS kind,
                name,
                parent,
                format AS file_type,
                hash,
                size_bytes,
                NULL AS sharing_mode,
                NULL AS share_expires_at,
                NULL AS preview_64,
                NULL AS preview_256,
                created_at,
                updated_at
            "#,
        )
        .bind(id)
        .bind(root_id)
        .fetch_all(executor)
        .await?;

        if restored.iter().all(|row| row.id != id) {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        Ok(restored)
    }

    /// Delete every trashed row in `root_id`. Returns the removed rows.
    ///
    /// Does not touch `file_metadata` because the same IDs are reused on restore.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_all<'e, E>(
        &self,
        executor: E,
        root_id: Uuid,
    ) -> Result<Vec<FileEntry>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query_as::<_, FileEntry>(
            r#"
            DELETE FROM trashed_files
            WHERE root_id = $1
            RETURNING
                id,
                root_id,
                created_by,
                "type" AS kind,
                name,
                parent,
                format AS file_type,
                hash,
                size_bytes,
                NULL AS sharing_mode,
                NULL AS share_expires_at,
                NULL AS preview_64,
                NULL AS preview_256,
                created_at,
                updated_at
            "#,
        )
        .bind(root_id)
        .fetch_all(executor)
        .await
        .map_err(AppError::from)
    }

    /// Delete trashed rows for `ids`. Returns the ids that were actually removed.
    ///
    /// Also removes matching `file_metadata` rows (hard delete).
    #[instrument(skip(self, ids), err(Debug))]
    pub async fn delete_many(&self, ids: &[Uuid]) -> Result<Vec<Uuid>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin().await?;

        let mut query = QueryBuilder::<Db>::new("DELETE FROM trashed_files WHERE id IN (");
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
        sqlx::query_as!(
            FileRef,
            r#"
            SELECT id
            FROM trashed_files
            WHERE root_id = $1
            "#,
            root_id
        )
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
        sqlx::query_as!(
            FileRef,
            r#"
            WITH RECURSIVE tree AS (
                SELECT id
                FROM trashed_files
                WHERE id = $1
                  AND root_id = $2

                UNION ALL

                SELECT child.id
                FROM trashed_files child
                JOIN tree parent ON child.parent = parent.id
            )
            SELECT id AS "id!"
            FROM tree
            "#,
            id,
            root_id
        )
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
        sqlx::query_as!(
            FileRef,
            r#"
            SELECT id
            FROM trashed_files
            WHERE trashed_at < $1
            "#,
            cutoff
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }
}
