use std::collections::HashSet;

use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::{
        format::FileType,
        model::{FileEntry, FileKind, Root, RootView},
        tree::FileEntryTree,
    },
};
use chrono::{DateTime, Utc};
use sqlx::{Executor, QueryBuilder};
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub(crate) enum FileTable {
    Live,
    Trashed,
}

impl FileTable {
    fn name(self) -> &'static str {
        match self {
            Self::Live => "files",
            Self::Trashed => "trashed_files",
        }
    }
}

#[derive(Clone)]
pub struct FileRepository {
    pool: DbPool,
}

pub struct NewFile {
    pub id: Uuid,
    pub root_id: Uuid,
    pub created_by: Uuid,
    pub kind: FileKind,
    pub name: String,
    /// `None` for an entry that sits directly in the root.
    pub parent: Option<Uuid>,
    pub file_type: Option<FileType>,
    pub hash: Option<String>,
    pub size_bytes: i64,
}

impl FileRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self, executor), fields(root_id = %id, name = %name), err(Debug))]
    pub async fn insert_root<'e, E>(
        &self,
        executor: E,
        id: Uuid,
        name: &str,
        quota: i64,
        owner: Option<Uuid>,
    ) -> Result<Root, AppError>
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
                owner,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $5)
            RETURNING 
                id,
                name,
                quota_bytes AS quota,
                owner,
                created_at,
                updated_at
            "#,
            id,
            name,
            quota,
            owner,
            now
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_private_roots(&self) -> Result<Vec<RootView>, AppError> {
        sqlx::query_as!(
            RootView,
            r#"
            SELECT
                id,
                name,
                quota_bytes AS quota,
                owner,
                created_at,
                updated_at
            FROM roots
            WHERE owner IS NOT NULL
            ORDER BY name
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_shared_roots(&self) -> Result<Vec<RootView>, AppError> {
        sqlx::query_as!(
            RootView,
            r#"
            SELECT
                id,
                name,
                quota_bytes AS quota,
                owner,
                created_at,
                updated_at
            FROM roots
            WHERE owner IS NULL
            ORDER BY name
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    /// Private root plus every shared root this user belongs to. The private root comes first.
    #[instrument(skip(self), err(Debug))]
    pub async fn list_accessible_roots(&self, user_id: Uuid) -> Result<Vec<RootView>, AppError> {
        sqlx::query_as!(
            RootView,
            r#"
            SELECT
                id,
                name,
                quota_bytes AS quota,
                owner,
                created_at,
                updated_at
            FROM roots
            WHERE owner = $1
            OR id IN (
                 SELECT root_id
                 FROM user_folders
                 WHERE user_id = $1
            )
            ORDER BY owner IS NULL, name
            "#,
            user_id
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
                owner,
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
        .ok_or_else(|| AppError::ResourceMissing("Folder does not exist".into()))
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
            return Err(AppError::ResourceMissing("Folder does not exist".into()));
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
                owner,
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
                id,
                name,
                quota_bytes AS quota,
                owner,
                created_at,
                updated_at 
            FROM roots
            WHERE owner = $1
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
                format AS "file_type: FileType",
                hash,
                size_bytes,
                created_at,
                updated_at
            FROM files
            WHERE id = $1
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
                format,
                hash,
                size_bytes,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING
                id,
                root_id,
                created_by,
                "type" AS "kind: FileKind",
                name,
                parent,
                format AS "file_type: FileType",
                hash,
                size_bytes,
                created_at,
                updated_at
            "#,
            file.id,
            file.root_id,
            file.created_by,
            file.kind as _,
            file.name,
            file.parent,
            file.file_type as _,
            file.hash,
            file.size_bytes,
            now,
            now
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    /// Insert every row in one statement. Parents and children may appear in any order.
    #[instrument(skip(self, executor, files), err(Debug))]
    pub async fn insert_many<'e, E>(
        &self,
        executor: E,
        files: Vec<FileEntry>,
    ) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        if files.is_empty() {
            return Ok(());
        }

        let mut insert = QueryBuilder::<Db>::new(
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
                created_at,
                updated_at
            )
            "#,
        );

        insert.push_values(&files, |mut row, file| {
            row.push_bind(file.id)
                .push_bind(file.root_id)
                .push_bind(file.created_by)
                .push_bind(file.kind)
                .push_bind(&file.name)
                .push_bind(file.parent)
                .push_bind(file.file_type.clone())
                .push_bind(&file.hash)
                .push_bind(file.size_bytes)
                .push_bind(file.created_at)
                .push_bind(file.updated_at);
        });

        insert
            .build()
            .execute(executor)
            .await
            .map_err(AppError::from)?;

        Ok(())
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
                format AS file_type,
                hash,
                size_bytes,
                created_at,
                updated_at
            FROM files
            WHERE
                root_id =
            "#,
        );

        query.push_bind(root);

        // Root entries have no parent. Everything else belongs to a directory.
        if let Some(parent) = parent {
            query.push(" AND parent = ").push_bind(parent);
        } else {
            query.push(" AND parent IS NULL");
        }

        query.push(r#" ORDER BY CASE WHEN "type" = 'd' THEN 0 ELSE 1 END, name, created_at, id"#);

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
                format AS "file_type: FileType",
                hash,
                size_bytes,
                created_at,
                updated_at
            FROM files
            WHERE root_id = $1
            "#,
            root
        )
        .fetch_all(executor)
        .await
        .map_err(AppError::from)
    }

    /// Descendants of a directory in `files`, down to the leaf nodes.
    pub async fn list_descendants(
        &self,
        root_id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<Vec<FileEntryTree>, AppError> {
        self.list_descendants_in(FileTable::Live, root_id, parent)
            .await
    }

    /// Trash for a root. Directly trashed rows have a null parent, so they are the roots.
    pub async fn list_trash(&self, root_id: Uuid) -> Result<Vec<FileEntryTree>, AppError> {
        self.list_descendants_in(FileTable::Trashed, root_id, None)
            .await
    }

    async fn list_descendants_in(
        &self,
        table: FileTable,
        root_id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<Vec<FileEntryTree>, AppError> {
        let table = table.name();
        let mut query = QueryBuilder::<Db>::new(format!(
            r#"
            WITH RECURSIVE tree AS (
                SELECT
                    id,
                    root_id,
                    "type" AS kind,
                    name,
                    parent,
                    format AS file_type,
                    hash,
                    size_bytes,
                    created_at,
                    updated_at,
                    TRUE AS query_root
                FROM {table}
                WHERE root_id =
            "#
        ));

        query.push_bind(root_id).push(" AND ");

        // `parent = $n` and `parent IS NULL` can each use an index.
        if let Some(parent) = parent {
            query.push("parent = ").push_bind(parent);
        } else {
            query.push("parent IS NULL");
        }

        query.push(format!(
            r#"
                UNION
                SELECT
                    f.id,
                    f.root_id,
                    f."type",
                    f.name,
                    f.parent,
                    f.format,
                    f.hash,
                    f.size_bytes,
                    f.created_at,
                    f.updated_at,
                    FALSE AS query_root
                FROM {table} f
                JOIN tree t ON f.parent = t.id
            )
            SELECT
                id,
                root_id,
                kind,
                name,
                parent,
                file_type,
                hash,
                size_bytes,
                created_at,
                updated_at,
                query_root
            FROM tree
            ORDER BY CASE WHEN kind = 'd' THEN 0 ELSE 1 END, name, created_at, id
            "#
        ));

        query
            .build_query_as::<FileEntryTree>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)
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
            "#,
            root_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
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

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn count_hash<'e, E>(&self, executor: E, hash: &str) -> Result<i64, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query_scalar!(
            r#"
            SELECT (
                (SELECT COUNT(*) FROM files WHERE hash = $1) +
                (SELECT COUNT(*) FROM trashed_files WHERE hash = $1)
            ) AS "count!"
            "#,
            hash
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    /// Delete trashed rows whose `trashed_at` is before `cutoff`. Hashes come back for blob cleanup.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_trashed_before<'e, E>(
        &self,
        executor: E,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<String>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let hashes = sqlx::query_scalar!(
            r#"
            DELETE FROM trashed_files
            WHERE trashed_at < $1
            RETURNING hash
            "#,
            cutoff
        )
        .fetch_all(executor)
        .await?;

        Ok(hashes.into_iter().flatten().collect())
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
                created_by,
                "type" AS "kind: FileKind",
                name,
                parent,
                format AS "file_type: FileType",
                hash,
                size_bytes,
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

    /// Trash roots in a root. Deleting these removes every trashed row in that root.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn list_trash_roots<'e, E>(
        &self,
        executor: E,
        root_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query_scalar!(
            r#"
            SELECT id AS "id!"
            FROM trashed_files
            WHERE root_id = $1
              AND parent IS NULL
            "#,
            root_id
        )
        .fetch_all(executor)
        .await
        .map_err(AppError::from)
    }

    /// Deletes `ids` and every row beneath them.
    ///
    /// The recursive term is a select. `DELETE RETURNING` is the statement that uses it, which
    /// Postgres and SQLite both accept. Hashes come back from the deleted rows; directories have none.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_tree<'e, E>(
        &self,
        executor: E,
        table: FileTable,
        ids: &[Uuid],
    ) -> Result<Vec<String>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let table = table.name();
        let mut query = QueryBuilder::<Db>::new(format!(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id
                FROM {table}
                WHERE id IN (
            "#
        ));

        let mut separated = query.separated(", ");

        for id in ids {
            separated.push_bind(*id);
        }

        separated.push_unseparated(format!(
            r#"
                )
                UNION ALL
                SELECT f.id
                FROM {table} f
                JOIN tree t ON f.parent = t.id
            )
            DELETE FROM {table}
            WHERE id IN (SELECT id FROM tree)
            RETURNING hash
            "#
        ));

        let hashes = query
            .build_query_scalar::<Option<String>>()
            .fetch_all(executor)
            .await?;

        Ok(hashes.into_iter().flatten().collect())
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
            return Err(AppError::ResourceMissing("Root does not exist".into()));
        }

        Ok(())
    }
}
