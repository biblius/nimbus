use std::fmt::Display;

use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::{format::FileType, model::FileKind, tree::FileEntryTree},
};
use sqlx::QueryBuilder;
use uuid::Uuid;

mod live;
mod trash;

pub use live::FileRepository;
pub use trash::TrashRepository;

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
    pub(crate) fn relation(self) -> &'static str {
        match self {
            Self::Live => "files",
            Self::Trashed => "trashed_files",
        }
    }
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

pub(crate) async fn list_descendants_in(
    pool: &DbPool,
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
                preview,
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
                f.preview,
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
            preview,
            created_at,
            updated_at,
            query_root
        FROM tree
        ORDER BY CASE WHEN kind = 'd' THEN 0 ELSE 1 END, name, created_at, id
        "#
    ));

    query
        .build_query_as::<FileEntryTree>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}
