use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::{format::FileType, model::FileKind, tree::FileEntryTree},
};
use sqlx::QueryBuilder;
use uuid::Uuid;

mod live;
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
                f.metadata,
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
                child.metadata,
                child.created_at,
                child.updated_at,
                FALSE AS query_root
            FROM {relation} child
            JOIN tree t ON child.parent = t.id
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
            metadata,
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
