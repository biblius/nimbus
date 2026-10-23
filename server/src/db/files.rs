use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::{format::FileType, model::FileKind, tree::FileEntryTree},
};
use sqlx::QueryBuilder;
use uuid::Uuid;

mod live;
mod search;
mod share;
mod trash;

pub use live::FileRepository;
pub use share::ShareRepository;
pub use trash::TrashRepository;

/// Which table a hard-deleted file lives in.
#[derive(Clone, Copy, Debug)]
pub(crate) enum FileTable {
    Live,
    Trashed,
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
                f.id,
                f.root_id,
                f."type" AS kind,
                f.name,
                f.parent,
                f.format AS file_type,
                f.hash,
                f.size_bytes,
                f.created_at,
                f.updated_at,
                TRUE AS query_root
            FROM {relation} f
            WHERE f.root_id =
        "#
    ));

    query.push_bind(root_id).push(" AND ");

    // `parent = $n` and `parent IS NULL` can each use an index.
    if let Some(parent) = parent {
        query.push("f.parent = ").push_bind(parent);
    } else {
        query.push("f.parent IS NULL");
    }

    query.push(format!(
        r#"
            UNION
            SELECT
                child.id,
                child.root_id,
                child."type",
                child.name,
                child.parent,
                child.format,
                child.hash,
                child.size_bytes,
                child.created_at,
                child.updated_at,
                FALSE AS query_root
            FROM {relation} child
            JOIN tree t ON child.parent = t.id
        )
        SELECT
            t.id,
            t.root_id,
            t.kind,
            t.name,
            t.parent,
            t.file_type,
            t.hash,
            t.size_bytes,
            m.preview_64,
            m.preview_256,
            m.preview_blob,
            t.created_at,
            t.updated_at,
            t.query_root
        FROM tree t
        LEFT JOIN file_metadata m ON m.file_id = t.id
        ORDER BY CASE WHEN t.kind = 'd' THEN 0 ELSE 1 END, t.name, t.created_at, t.id
        "#
    ));

    query
        .build_query_as::<FileEntryTree>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}
