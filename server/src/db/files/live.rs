use crate::{
    db::{
        Db, DbPool,
        files::{FileTable, NewFile, list_descendants_in},
    },
    error::AppError,
    file::{
        format::FileType,
        model::{FileEntry, FileMetadata, FileRef, FileView, MAX_ANCESTOR_DEPTH, Root, RootView},
        tree::FileEntryTree,
    },
};
use chrono::{DateTime, Utc};
use sqlx::{Executor, FromRow, QueryBuilder};
use std::collections::{HashMap, HashSet};
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct HashCount {
    hash: String,
    count: i64,
}

#[derive(Debug, FromRow)]
struct PreviewCount {
    preview: String,
    count: i64,
}

#[derive(Clone)]
pub struct FileRepository {
    pool: DbPool,
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
                id AS "id: Uuid",
                name,
                quota_bytes AS quota,
                owner AS "owner?: Uuid",
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>"
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
                id AS "id: Uuid",
                name,
                quota_bytes AS quota,
                owner AS "owner?: Uuid",
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>"
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
                id AS "id: Uuid",
                name,
                quota_bytes AS quota,
                owner AS "owner?: Uuid",
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>"
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
                id AS "id: Uuid",
                name,
                quota_bytes AS quota,
                owner AS "owner?: Uuid",
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>"
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
                id AS "id: Uuid",
                name,
                quota_bytes AS quota,
                owner AS "owner?: Uuid",
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>"
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
                id AS "id: Uuid",
                name,
                quota_bytes AS quota,
                owner AS "owner?: Uuid",
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>"
            FROM roots WHERE id = $1"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn get_private_root(&self, user_id: Uuid) -> Result<Root, AppError> {
        sqlx::query_as!(
            Root,
            r#"
            SELECT
                id AS "id: Uuid",
                name,
                quota_bytes AS quota,
                owner AS "owner?: Uuid",
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>"
            FROM roots
            WHERE owner = $1
            "#,
            user_id
        )
        .fetch_one(&self.pool)
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
        self.get_live_with(&self.pool, id).await
    }

    /// Live rows for every id in `ids`. Missing ids are omitted.
    #[instrument(skip(self, ids), err(Debug))]
    pub async fn get_live_many(&self, ids: &[Uuid]) -> Result<Vec<FileEntry>, AppError> {
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
                f.sharing_mode,
                f.share_expires_at,
                m.preview_64,
                m.preview_256,
                f.created_at,
                f.updated_at
            FROM files f
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

    /// Ancestors of `id` in `root_id`, ordered root → … → `id` (inclusive).
    ///
    /// Empty when `id` is missing or not in `root_id`. Walks at most
    /// [`MAX_ANCESTOR_DEPTH`] hops (defensive; the app never creates cycles).
    #[instrument(skip(self), err(Debug))]
    pub async fn list_ancestors(
        &self,
        root_id: Uuid,
        id: Uuid,
    ) -> Result<Vec<FileEntry>, AppError> {
        let mut query = QueryBuilder::<Db>::new(
            r#"
            WITH RECURSIVE tree AS (
                SELECT
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
                    updated_at,
                    0 AS depth
                FROM files
                WHERE id =
            "#,
        );
        query.push_bind(id);
        query.push(" AND root_id = ").push_bind(root_id);
        query.push(
            r#"
                UNION ALL
                SELECT
                    p.id,
                    p.root_id,
                    p.created_by,
                    p."type",
                    p.name,
                    p.parent,
                    p.format,
                    p.hash,
                    p.size_bytes,
                    p.sharing_mode,
                    p.share_expires_at,
                    p.created_at,
                    p.updated_at,
                    t.depth + 1
                FROM files p
                JOIN tree t ON p.id = t.parent
                 AND p.root_id = t.root_id
                WHERE t.depth <
            "#,
        );
        query.push_bind(MAX_ANCESTOR_DEPTH);
        query.push(
            r#"
            )
            SELECT
                t.id,
                t.root_id,
                t.created_by,
                t."type" AS kind,
                t.name,
                t.parent,
                t.format AS file_type,
                t.hash,
                t.size_bytes,
                t.sharing_mode,
                t.share_expires_at,
                m.preview_64,
                m.preview_256,
                t.created_at,
                t.updated_at
            FROM tree t
            LEFT JOIN file_metadata m ON m.file_id = t.id
            ORDER BY t.depth DESC
            "#,
        );

        query
            .build_query_as::<FileEntry>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn get_live_with<'e, E>(
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
                f.sharing_mode,
                f.share_expires_at,
                m.preview_64,
                m.preview_256,
                f.created_at,
                f.updated_at
            FROM files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE f.id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(executor)
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

        // Plain INSERT RETURNING (no modifying CTE) so this works on SQLite.
        // New rows have no file_metadata yet so preview columns are null.
        sqlx::query_as::<_, FileEntry>(
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
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                (SELECT p.sharing_mode FROM files p WHERE p.id = $6),
                (SELECT p.share_expires_at FROM files p WHERE p.id = $6),
                $10, $11
            )
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
                CAST(NULL AS TEXT) AS preview_64,
                CAST(NULL AS TEXT) AS preview_256,
                created_at,
                updated_at
            "#,
        )
        .bind(file.id)
        .bind(file.root_id)
        .bind(file.created_by)
        .bind(file.kind)
        .bind(&file.name)
        .bind(file.parent)
        .bind(&file.file_type)
        .bind(&file.hash)
        .bind(file.size_bytes)
        .bind(now)
        .bind(now)
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    /// Insert every row in one statement. Parents and children may appear in any order.
    #[instrument(skip(self, conn, files), err(Debug))]
    pub async fn insert_many(
        &self,
        conn: &mut <Db as sqlx::Database>::Connection,
        files: &[FileEntry],
    ) -> Result<(), AppError> {
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
                sharing_mode,
                share_expires_at,
                created_at,
                updated_at
            )
            "#,
        );

        insert.push_values(files, |mut row, file| {
            row.push_bind(file.id)
                .push_bind(file.root_id)
                .push_bind(file.created_by)
                .push_bind(file.kind)
                .push_bind(&file.name)
                .push_bind(file.parent)
                .push_bind(&file.file_type)
                .push_bind(&file.hash)
                .push_bind(file.size_bytes)
                .push_bind(file.sharing_mode)
                .push_bind(file.share_expires_at)
                .push_bind(file.created_at)
                .push_bind(file.updated_at);
        });

        insert
            .build()
            .execute(&mut *conn)
            .await
            .map_err(AppError::from)?;

        let file_metadata: Vec<_> = files
            .iter()
            .filter(|file| !file.metadata.is_empty())
            .map(|file| {
                (
                    file.id,
                    file.metadata.preview_64.as_deref(),
                    file.metadata.preview_256.as_deref(),
                )
            })
            .collect();

        if file_metadata.is_empty() {
            return Ok(());
        }

        let mut upsert = QueryBuilder::<Db>::new(
            "INSERT INTO file_metadata (file_id, preview_64, preview_256) ",
        );

        upsert.push_values(file_metadata, |mut row, (id, preview_64, preview_256)| {
            row.push_bind(id)
                .push_bind(preview_64)
                .push_bind(preview_256);
        });

        upsert.push(
            r#"
                ON CONFLICT (file_id) DO UPDATE SET
                    preview_64 = EXCLUDED.preview_64,
                    preview_256 = EXCLUDED.preview_256
                "#,
        );

        upsert
            .build()
            .execute(&mut *conn)
            .await
            .map_err(AppError::from)?;

        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_files(
        &self,
        root: Uuid,
        parent: Option<Uuid>,
    ) -> Result<Vec<FileView>, AppError> {
        self.list_files_with(&self.pool, root, parent).await
    }

    /// Live children of a directory.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn list_files_with<'e, E>(
        &self,
        executor: E,
        root: Uuid,
        parent: Option<Uuid>,
    ) -> Result<Vec<FileView>, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
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
                f.sharing_mode,
                f.share_expires_at,
                m.preview_64,
                m.preview_256,
                f.created_at,
                f.updated_at
            FROM files f
            LEFT JOIN file_metadata m ON m.file_id = f.id
            WHERE
                f.root_id =
            "#,
        );

        query.push_bind(root);

        // Root entries have no parent. Everything else belongs to a directory.
        if let Some(parent) = parent {
            query.push(" AND f.parent = ").push_bind(parent);
        } else {
            query.push(" AND f.parent IS NULL");
        }

        query.push(
            r#" ORDER BY CASE WHEN f."type" = 'd' THEN 0 ELSE 1 END, f.name, f.created_at, f.id"#,
        );

        let rows = query
            .build_query_as::<FileEntry>()
            .fetch_all(executor)
            .await
            .map_err(AppError::from)?;

        Ok(rows.into_iter().map(FileView::from).collect())
    }

    /// Descendants of a directory in `files`, down to the leaf nodes.
    pub async fn list_descendants(
        &self,
        root_id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<Vec<FileEntryTree>, AppError> {
        list_descendants_in(&self.pool, FileTable::Live, root_id, parent).await
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

    /// Store generated preview metadata on a live file row.
    #[instrument(skip(self, metadata), fields(file_id = %file_id), err(Debug))]
    pub async fn set_metadata(
        &self,
        file_id: Uuid,
        metadata: &FileMetadata,
    ) -> Result<(), AppError> {
        let now = Utc::now();

        let result = sqlx::query!(
            r#"
            UPDATE files
            SET updated_at = $2
            WHERE id = $1
            "#,
            file_id,
            now
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        sqlx::query!(
            r#"
            INSERT INTO file_metadata (file_id, preview_64, preview_256)
            VALUES ($1, $2, $3)
            ON CONFLICT (file_id) DO UPDATE SET
                preview_64 = EXCLUDED.preview_64,
                preview_256 = EXCLUDED.preview_256
            "#,
            file_id,
            metadata.preview_64,
            metadata.preview_256
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Replace the content-addressed fields of an existing live file.
    #[instrument(skip(self, executor, file_type, hash), fields(file_id = %id), err(Debug))]
    pub async fn update_content<'e, E>(
        &self,
        executor: E,
        id: Uuid,
        hash: &str,
        size_bytes: i64,
        file_type: &FileType,
    ) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let now = Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE files
            SET
                hash = $2,
                size_bytes = $3,
                format = $4,
                updated_at = $5
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(hash)
        .bind(size_bytes)
        .bind(file_type)
        .bind(now)
        .execute(executor)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        Ok(())
    }

    /// Update only the display name of a live file or directory.
    #[instrument(skip(self, name), fields(file_id = %id, name = %name), err(Debug))]
    pub async fn update_name(&self, id: Uuid, name: &str) -> Result<(), AppError> {
        let now = Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE files
            SET
                name = $2,
                updated_at = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(now)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn count_hash(&self, hash: &str) -> Result<i64, AppError> {
        self.count_hash_with(&self.pool, hash).await
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn count_hash_with<'e, E>(&self, executor: E, hash: &str) -> Result<i64, AppError>
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

    /// Remaining live+trashed references for each content hash.
    /// Hashes with zero references are omitted from the map.
    #[instrument(skip(self, hashes), err(Debug))]
    pub async fn count_hashes(&self, hashes: &[&str]) -> Result<HashMap<String, i64>, AppError> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }

        let mut query = QueryBuilder::<Db>::new(
            r#"
            SELECT hash, COUNT(*) AS count
            FROM (
                SELECT hash FROM files WHERE hash IN (
            "#,
        );

        {
            let mut selected = query.separated(", ");
            for hash in hashes {
                selected.push_bind(hash);
            }
        }

        query.push(
            r#"
                )
                UNION ALL
                SELECT hash FROM trashed_files WHERE hash IN (
            "#,
        );

        {
            let mut selected = query.separated(", ");
            for hash in hashes {
                selected.push_bind(hash);
            }
        }

        query.push(
            r#"
                )
            ) AS refs
            WHERE hash IS NOT NULL
            GROUP BY hash
            "#,
        );

        let rows = query
            .build_query_as::<HashCount>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(rows.into_iter().map(|row| (row.hash, row.count)).collect())
    }

    /// Remaining references for each preview hash across `file_metadata`.
    /// Previews with zero references are omitted from the map.
    #[instrument(skip(self, hashes), err(Debug))]
    pub async fn count_previews(&self, hashes: &[&str]) -> Result<HashMap<String, i64>, AppError> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }

        let mut query = QueryBuilder::<Db>::new(
            r#"
            SELECT preview, COUNT(*) AS count
            FROM (
                SELECT preview_64 AS preview
                FROM file_metadata
                WHERE preview_64 IN (
            "#,
        );

        {
            let mut selected = query.separated(", ");
            for preview in hashes {
                selected.push_bind(preview);
            }
        }

        query.push(
            r#"
                )
                UNION ALL
                SELECT preview_256
                FROM file_metadata
                WHERE preview_256 IN (
            "#,
        );

        {
            let mut selected = query.separated(", ");
            for preview in hashes {
                selected.push_bind(preview);
            }
        }

        query.push(
            r#"
                )
            ) AS refs
            WHERE preview IS NOT NULL
            GROUP BY preview
            "#,
        );

        let rows = query
            .build_query_as::<PreviewCount>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(|row| (row.preview, row.count))
            .collect())
    }

    /// List files in `ids` and all of their descendants in `root_id`.
    ///
    /// Overlapping selections should never happen
    /// (e.g. both a directory and its child are in `ids`), but
    /// are deduped by `UNION` just in case.
    #[instrument(skip(self, ids), err(Debug))]
    pub async fn list_with_subtrees(
        &self,
        root_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<FileEntry>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query = QueryBuilder::<Db>::new(
            r#"
            WITH RECURSIVE tree AS (
                SELECT
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
                FROM files
                WHERE root_id =
            "#,
        );

        query.push_bind(root_id);
        query.push(" AND id IN (");

        {
            let mut selected = query.separated(", ");
            for id in ids {
                selected.push_bind(*id);
            }
        }

        query.push(
            r#"
                )
                UNION
                SELECT
                    child.id,
                    child.root_id,
                    child.created_by,
                    child."type",
                    child.name,
                    child.parent,
                    child.format,
                    child.hash,
                    child.size_bytes,
                    child.sharing_mode,
                    child.share_expires_at,
                    child.created_at,
                    child.updated_at
                FROM files child
                JOIN tree t ON child.parent = t.id
            )
            SELECT
                t.id,
                t.root_id,
                t.created_by,
                t."type" AS kind,
                t.name,
                t.parent,
                t.format AS file_type,
                t.hash,
                t.size_bytes,
                t.sharing_mode,
                t.share_expires_at,
                m.preview_64,
                m.preview_256,
                t.created_at,
                t.updated_at
            FROM tree t
            LEFT JOIN file_metadata m ON m.file_id = t.id
            "#,
        );

        let rows = query
            .build_query_as::<FileEntry>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        let found: HashSet<_> = rows.iter().map(|row| row.id).collect();

        if ids.iter().any(|id| !found.contains(id)) {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

        Ok(rows)
    }

    /// Set `parent` on live rows in one root. Trashed rows are not updated.
    #[instrument(skip(self, ids), err(Debug))]
    pub async fn move_to_parent(
        &self,
        root_id: Uuid,
        parent: Option<Uuid>,
        ids: &[Uuid],
    ) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }

        let mut query = QueryBuilder::<Db>::new("UPDATE files SET parent = ");
        query.push_bind(parent);
        query.push(", updated_at = ");
        query.push_bind(Utc::now());
        query.push(" WHERE root_id = ");
        query.push_bind(root_id);
        query.push(" AND id IN (");

        let mut separated = query.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }

        query.push(")");

        let affected = query
            .build()
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?
            .rows_affected();

        if ids.len() as u64 != affected {
            tracing::warn!("some files were not moved")
        }

        Ok(())
    }

    /// Delete rows for `ids`. Returns the ids that were actually removed.
    ///
    /// Also removes matching `file_metadata` rows (hard delete).
    #[instrument(skip(self, ids), err(Debug))]
    pub async fn delete_many(&self, ids: &[Uuid]) -> Result<Vec<Uuid>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin().await?;

        let mut query = QueryBuilder::<Db>::new("DELETE FROM files WHERE id IN (");
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

    pub async fn delete_root(&self, id: Uuid) -> Result<(), AppError> {
        self.delete_root_with(&self.pool, id).await
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_root_with<'e, E>(&self, executor: E, id: Uuid) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let result = sqlx::query!(
            r#"
            DELETE FROM roots WHERE id = $1
            "#,
            id
        )
        .execute(executor)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("Root does not exist".into()));
        }

        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_file_ids(&self, root_id: Uuid) -> Result<Vec<FileRef>, AppError> {
        sqlx::query_as!(
            FileRef,
            r#"
            SELECT id AS "id: Uuid"
            FROM files
            WHERE root_id = $1
            "#,
            root_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }
}
