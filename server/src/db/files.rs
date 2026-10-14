use std::{collections::HashSet, fmt::Display};

use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::{
        format::FileType,
        model::{FileEntry, FileKind, FileMetadata, FileRef, FileView, Root, RootView},
        tree::FileEntryTree,
    },
};
use chrono::{DateTime, Utc};
use sqlx::{Executor, FromRow, QueryBuilder};
use tracing::instrument;
use uuid::Uuid;

/// Which table a queued/deleted file lives in. Stored as text: `files` or `trash`.
#[derive(Clone, Copy, Debug, sqlx::Type)]
#[sqlx(type_name = "text")]
pub(crate) enum FileTable {
    #[sqlx(rename = "files")]
    Live,
    #[sqlx(rename = "trash")]
    Trashed,
}

impl Display for FileTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Queue `origin` column values (`files` | `trash`), not SQL relation names.
        match self {
            Self::Live => write!(f, "files"),
            Self::Trashed => write!(f, "trash"),
        }
    }
}

impl FileTable {
    /// Actual relation name for queries (`files` | `trashed_files`).
    fn relation(self) -> &'static str {
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

/// Directory listing row (includes `preview` from `files`).
#[derive(Debug, FromRow)]
struct FileViewRow {
    id: Uuid,
    root_id: Uuid,
    created_by: Option<Uuid>,
    kind: FileKind,
    name: String,
    parent: Option<Uuid>,
    file_type: Option<FileType>,
    hash: Option<String>,
    size_bytes: i64,
    preview: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<FileViewRow> for FileView {
    fn from(row: FileViewRow) -> Self {
        let preview = row.preview.clone();
        let metadata = preview.as_ref().map(|hash| FileMetadata {
            preview: Some(hash.clone()),
        });

        FileView::new(
            FileEntry {
                id: row.id,
                root_id: row.root_id,
                created_by: row.created_by,
                kind: row.kind,
                name: row.name,
                parent: row.parent,
                file_type: row.file_type,
                hash: row.hash,
                size_bytes: row.size_bytes,
                preview,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            metadata,
        )
    }
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

    #[instrument(skip(self), err(Debug))]
    pub async fn get_private_root(&self, user_id: Uuid) -> Result<Root, AppError> {
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

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn get_live_with<'e, E>(
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
                preview,
                created_at,
                updated_at
            FROM files
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(executor)
        .await
        .map_err(AppError::from)
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
                preview,
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
                preview,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, $10, $11)
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
                preview,
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
                preview,
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
                .push_bind(&file.preview)
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
                f.preview,
                f.created_at,
                f.updated_at
            FROM files f
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
            .build_query_as::<FileViewRow>()
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
        let relation = table.relation();
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
                FROM {relation}
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
                FROM {relation} f
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
                preview,
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
                preview,
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
                .push_bind(&file.preview)
                .push_bind(file.created_at)
                .push_bind(now)
                .push_bind(now);
        });

        insert.build().execute(&mut *tx).await?;

        tx.commit().await?;

        Ok(())
    }

    /// Store the content hash of a generated preview on a live file row.
    #[instrument(skip(self), fields(file_id = %file_id), err(Debug))]
    pub async fn set_preview(&self, file_id: Uuid, preview: &str) -> Result<(), AppError> {
        let now = Utc::now();

        let result = sqlx::query!(
            r#"
            UPDATE files
            SET preview = $2, updated_at = $3
            WHERE id = $1
            "#,
            file_id,
            preview,
            now
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("File does not exist".into()));
        }

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
    ) -> Result<FileEntry, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let now = Utc::now();

        sqlx::query_as::<_, FileEntry>(
            r#"
            UPDATE files
            SET
                hash = $2,
                size_bytes = $3,
                format = $4,
                updated_at = $5
            WHERE id = $1
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
                preview,
                created_at,
                updated_at
            "#,
        )
        .bind(id)
        .bind(hash)
        .bind(size_bytes)
        .bind(file_type)
        .bind(now)
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
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

    #[instrument(skip(self), err(Debug))]
    pub async fn count_preview(&self, preview: &str) -> Result<i64, AppError> {
        self.count_preview_with(&self.pool, preview).await
    }

    /// How many live/trashed rows still point at this preview blob.
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn count_preview_with<'e, E>(
        &self,
        executor: E,
        preview: &str,
    ) -> Result<i64, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query_scalar!(
            r#"
            SELECT (
                (SELECT COUNT(*) FROM files WHERE preview = $1) +
                (SELECT COUNT(*) FROM trashed_files WHERE preview = $1)
            ) AS "count!"
            "#,
            preview
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
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
                preview,
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
    pub async fn delete_live(&self, id: Uuid) -> Result<(), AppError> {
        self.delete_live_with(&self.pool, id).await
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_live_with<'e, E>(&self, executor: E, id: Uuid) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query!(r#"DELETE FROM files WHERE id = $1"#, id)
            .execute(executor)
            .await?;

        Ok(())
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

    pub async fn delete_root(&self, id: Uuid) -> Result<(), AppError> {
        self.delete_root_with(&self.pool, id).await
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

    #[instrument(skip(self), err(Debug))]
    pub async fn list_file_ids(&self, root_id: Uuid) -> Result<Vec<FileRef>, AppError> {
        sqlx::query_as!(
            FileRef,
            r#"
            SELECT id
            FROM files
            WHERE root_id = $1
            "#,
            root_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
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
