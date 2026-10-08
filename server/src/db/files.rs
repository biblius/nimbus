use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::{
        format::{FileFamily, FileFormat},
        model::{FileEntry, FileKind, Root},
    },
};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::{Executor, QueryBuilder};
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct FileRepository {
    pool: DbPool,
}

pub struct NewRoot {
    pub id: Uuid,
    pub name: String,
    pub quota: i64,
}

#[derive(FromRow)]
struct DeletedHash {
    hash: Option<String>,
}

pub struct NewFile {
    pub id: Uuid,
    pub root_id: Uuid,
    pub created_by: Uuid,
    pub kind: FileKind,
    pub name: String,
    /// `None` for an entry that sits directly in the root.
    pub parent: Option<Uuid>,
    pub family: Option<FileFamily>,
    pub format: Option<FileFormat>,
    pub hash: Option<String>,
    pub size_bytes: i64,
}

impl FileRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self, executor, root), fields(root_id = %root.id, name = %root.name), err(Debug))]
    pub async fn insert_root<'e, E>(&self, executor: E, root: NewRoot) -> Result<Root, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let now = Utc::now();

        sqlx::query_as!(
            Root,
            r#"
            INSERT INTO roots (
                id,
                name,
                quota_bytes,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $4)
            RETURNING 
                id,
                name,
                quota_bytes AS quota,
                created_at,
                updated_at
            "#,
            root.id,
            root.name,
            root.quota,
            now
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_all_roots(&self) -> Result<Vec<Root>, AppError> {
        sqlx::query_as!(
            Root,
            r#"
            SELECT
                id,
                name,
                quota_bytes AS quota,
                created_at,
                updated_at
            FROM roots
            ORDER BY name
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn update_root(
        &self,
        id: Uuid,
        name: &str,
        quota_bytes: i64,
    ) -> Result<Root, AppError> {
        let now = Utc::now();

        sqlx::query_as!(
            Root,
            r#"
            UPDATE roots
            SET
                name = $1,
                quota_bytes = $2,
                updated_at = $3
            WHERE id = $4
            RETURNING
                id,
                name,
                quota_bytes AS quota,
                created_at,
                updated_at
            "#,
            name,
            quota_bytes,
            now,
            id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::ResourceMissing("folder does not exist".into()))
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn set_quota(&self, root_id: Uuid, quota_bytes: i64) -> Result<(), AppError> {
        let now = Utc::now();

        let updated = sqlx::query!(
            r#"
            UPDATE roots
            SET
                quota_bytes = $1,
                updated_at = $2
            WHERE id = $3
            "#,
            quota_bytes,
            now,
            root_id
        )
        .execute(&self.pool)
        .await?
        .rows_affected();

        if updated == 0 {
            return Err(AppError::ResourceMissing("folder does not exist".into()));
        }

        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn get_root(&self, id: Uuid) -> Result<Option<Root>, AppError> {
        sqlx::query_as!(
            Root,
            r#"
            SELECT 
                id,
                name,
                quota_bytes AS quota,
                created_at,
                updated_at 
            FROM roots WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn get_private_root_with<'e, E>(
        &self,
        executor: E,
        user_id: Uuid,
    ) -> Result<Root, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query_as!(
            Root,
            r#"
            SELECT 
                r.id,
                r.name,
                r.quota_bytes AS quota,
                r.created_at,
                r.updated_at 
            FROM user_folders uf 
            JOIN roots r ON r.id = uf.root_id
            WHERE 
                user_id = $1
            AND uf."primary"
            "#,
            user_id
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    /// Quota on this user's assignment. `0` is unlimited.
    #[instrument(skip(self), err(Debug))]
    pub async fn root_quota(&self, root_id: Uuid) -> Result<i64, AppError> {
        sqlx::query_scalar!(
            r#"
            SELECT quota_bytes 
            FROM roots 
            WHERE id = $1 
            "#,
            root_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            tracing::error!(%root_id, "quota requested without an assignment");
            AppError::Internal
        })
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn get_live(&self, id: Uuid) -> Result<Option<FileEntry>, AppError> {
        sqlx::query_as!(
            FileEntry,
            r#"
            SELECT
                id,
                root_id,
                created_by,
                "type" AS "kind: FileKind",
                name,
                parent,
                family AS "family: FileFamily",
                format AS "format: FileFormat",
                hash,
                size_bytes,
                created_at,
                updated_at,
                deleted_at
            FROM files
            WHERE 
                id = $1
            AND deleted_at IS NULL
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self, node), fields(file_id = %node.id, root_id = %node.root_id, name = %node.name), err(Debug))]
    pub async fn create(&self, node: NewFile) -> Result<FileEntry, AppError> {
        self.insert(&self.pool, node).await
    }

    #[instrument(skip(self, executor, file), fields(file_id = %file.id, root_id = %file.root_id, name = %file.name), err(Debug))]
    pub async fn insert<'e, E>(&self, executor: E, file: NewFile) -> Result<FileEntry, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let now = Utc::now();

        sqlx::query_as!(
            FileEntry,
            r#"
            INSERT INTO files (
                id,
                root_id,
                created_by,
                "type",
                name,
                parent,
                family,
                format,
                hash,
                size_bytes,
                created_at,
                updated_at,
                deleted_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11, NULL)
            RETURNING
                id,
                root_id,
                created_by,
                "type" AS "kind: FileKind",
                name,
                parent,
                family AS "family: FileFamily",
                format AS "format: FileFormat",
                hash,
                size_bytes,
                created_at,
                updated_at,
                deleted_at
            "#,
            file.id,
            file.root_id,
            file.created_by,
            file.kind as _,
            file.name,
            file.parent,
            file.family.map(|f| f.to_string()),
            file.format.map(|f| f.to_string()),
            file.hash,
            file.size_bytes,
            now
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_files(
        &self,
        root: Uuid,
        parent: Option<Uuid>,
    ) -> Result<Vec<FileEntry>, AppError> {
        self.list_files_with(&self.pool, root, parent).await
    }

    /// Live children of a directory.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn list_files_with<'e, E>(
        &self,
        executor: E,
        root: Uuid,
        parent: Option<Uuid>,
    ) -> Result<Vec<FileEntry>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let mut query = QueryBuilder::new(
            r#"
            SELECT
                id,
                root_id,
                created_by,
                "type" AS kind,
                name,
                parent,
                family,
                format,
                hash,
                size_bytes,
                created_at,
                updated_at,
                deleted_at
            FROM files
            WHERE
                root_id =
            "#,
        );

        query.push_bind(root).push(" AND deleted_at IS NULL");

        if let Some(parent) = parent {
            query.push(" AND parent = ").push_bind(parent);
        };

        query.push(r#" ORDER BY CASE WHEN "type" = 'd' THEN 0 ELSE 1 END, name"#);

        query
            .build_query_as::<FileEntry>()
            .fetch_all(executor)
            .await
            .map_err(AppError::from)
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn list_all_files_with<'e, E>(
        &self,
        executor: E,
        root: Uuid,
    ) -> Result<Vec<FileEntry>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query_as!(
            FileEntry,
            r#"
            SELECT
                id,
                root_id,
                created_by,
                "type" AS "kind: FileKind",
                name,
                parent,
                family AS "family: FileFamily",
                format AS "format: FileFormat",
                hash,
                size_bytes,
                created_at,
                updated_at,
                deleted_at
            FROM files
            WHERE root_id = $1
            "#,
            root
        )
        .fetch_all(executor)
        .await
        .map_err(AppError::from)
    }

    /// Every child of a directory, including soft-deleted rows.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn list_all_children<'e, E>(
        &self,
        executor: E,
        parent: Uuid,
    ) -> Result<Vec<FileEntry>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query_as!(
            FileEntry,
            r#"
            SELECT
                id,
                root_id,
                created_by,
                "type" AS "kind: FileKind",
                name,
                parent,
                family AS "family: FileFamily",
                format AS "format: FileFormat",
                hash,
                size_bytes,
                created_at,
                updated_at,
                deleted_at
            FROM files
            WHERE parent = $1
            "#,
            parent
        )
        .fetch_all(executor)
        .await
        .map_err(AppError::from)
    }

    /// `parent` is null when checking the top level of a root.
    #[instrument(skip(self), err(Debug))]
    pub async fn name_taken(
        &self,
        root_id: Uuid,
        parent: Option<Uuid>,
        name: &str,
    ) -> Result<bool, AppError> {
        let found = sqlx::query_scalar!(
            r#"
            SELECT id
            FROM files
            WHERE root_id = $1
              AND name = $2
              AND deleted_at IS NULL
              AND parent IS NOT DISTINCT FROM $3
            "#,
            root_id,
            name,
            parent
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(found.is_some())
    }

    /// Bytes stored in a root. Directory rows are excluded.
    #[instrument(skip(self), err(Debug))]
    pub async fn root_quota_usage(&self, root_id: Uuid) -> Result<i64, AppError> {
        sqlx::query_scalar!(
            r#"
            SELECT CAST(COALESCE(SUM(size_bytes), 0) AS BIGINT) AS "usage!"
            FROM files
            WHERE root_id = $1
              AND "type" = 'f'
              AND deleted_at IS NULL
            "#,
            root_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn soft_delete(&self, ids: &[Uuid]) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }

        let now = Utc::now();

        let mut query = QueryBuilder::<Db>::new("UPDATE files SET deleted_at = ");

        query.push_bind(now);
        query.push(", updated_at = ");
        query.push_bind(now);
        query.push(" WHERE deleted_at IS NULL AND id IN (");

        let mut separated = query.separated(", ");

        for id in ids {
            separated.push_bind(*id);
        }

        separated.push_unseparated(")");

        let result = query.build().execute(&self.pool).await?;

        if result.rows_affected() == 0 {
            tracing::warn!("soft delete did not touch any rows");
        }

        Ok(())
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn count_hash<'e, E>(&self, executor: E, hash: &str) -> Result<i64, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!"
            FROM files
            WHERE hash = $1
            "#,
            hash
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    /// Soft-deleted rows past the retention cutoff.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn list_pending_deletion<'e, E>(
        &self,
        executor: E,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<FileEntry>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query_as!(
            FileEntry,
            r#"
            SELECT
                id,
                root_id,
                created_by,
                "type" AS "kind: FileKind",
                name,
                parent,
                family AS "family: FileFamily",
                format AS "format: FileFormat",
                hash,
                size_bytes,
                created_at,
                updated_at,
                deleted_at
            FROM files
            WHERE deleted_at < $1
            "#,
            cutoff
        )
        .fetch_all(executor)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete<'e, E>(&self, executor: E, ids: &[Uuid]) -> Result<Vec<String>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query = QueryBuilder::<Db>::new("DELETE FROM files WHERE id IN (");

        let mut separated = query.separated(", ");

        for id in ids {
            separated.push_bind(*id);
        }

        separated.push_unseparated(") RETURNING hash");

        let hashes = query
            .build_query_as::<DeletedHash>()
            .fetch_all(executor)
            .await?;

        Ok(hashes
            .into_iter()
            .filter_map(|record| record.hash)
            .collect())
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_root_with<'e, E>(&self, executor: E, id: Uuid) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let result = sqlx::query!(r#"DELETE FROM roots WHERE id = $1"#, id)
            .execute(executor)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("root does not exist".into()));
        }

        Ok(())
    }
}
