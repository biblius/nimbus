use std::collections::HashSet;

use crate::{
    db::{
        Db, DbPool,
        files::{FileTable, list_descendants_in},
    },
    error::AppError,
    file::{
        format::FileType,
        model::{FileEntry, FileKind, FileMetadata, FileRef},
        tree::FileEntryTree,
    },
};
use chrono::{DateTime, Utc};
use sqlx::{Executor, QueryBuilder, types::Json};
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
                metadata AS "metadata: Json<FileMetadata>",
                created_at,
                updated_at
            FROM trashed_files
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(executor)
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
                metadata,
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
                metadata,
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
                .push_bind(&file.metadata)
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
    pub async fn trashed_root(&self, id: Uuid) -> Result<Option<Uuid>, AppError> {
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
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_trashed_tree<'e, E>(
        &self,
        executor: E,
        root_id: Uuid,
        id: Uuid,
    ) -> Result<Vec<FileEntry>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let restored = sqlx::query_as!(
            FileEntry,
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
                created_by AS "created_by?",
                "type" AS "kind: FileKind",
                name,
                parent,
                format AS "file_type: FileType",
                hash,
                size_bytes,
                metadata AS "metadata: Json<FileMetadata>",
                created_at,
                updated_at
            "#,
            id,
            root_id
        )
        .fetch_all(executor)
        .await?;

        if restored.iter().all(|row| row.id != id) {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        Ok(restored)
    }
    #[instrument(skip(self), err(Debug))]
    pub async fn delete_trashed(&self, id: Uuid) -> Result<(), AppError> {
        self.delete_trashed_with(&self.pool, id).await
    }
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_trashed_with<'e, E>(&self, executor: E, id: Uuid) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query!(r#"DELETE FROM trashed_files WHERE id = $1"#, id)
            .execute(executor)
            .await?;

        Ok(())
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
    /// Trashed rows past `cutoff`.
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
