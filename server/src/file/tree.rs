use std::collections::HashMap;

use crate::file::{
    format::FileType,
    model::{FileKind, FileMetadata},
};
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileTree {
    pub id: Uuid,
    pub name: String,
    pub kind: FileKind,
    /// Detected format for files; null for directories.
    pub file_type: Option<FileType>,
    /// Preview hashes from `file_metadata`, when present.
    pub metadata: Option<FileMetadata>,
    /// OpenAPI cannot expand this field; the value is the same `FileTree` shape.
    #[schema(no_recursion)]
    pub children: Vec<FileTree>,
}

impl FileTree {
    /// `query_root` rows are the roots. Every other row hangs off `parent`.
    pub fn from_entries(entries: Vec<FileEntryTree>) -> Vec<Self> {
        let mut roots = Vec::new();

        let mut by_parent: HashMap<Uuid, Vec<FileEntryTree>> = HashMap::new();

        for entry in entries {
            if entry.query_root {
                roots.push(entry);
            } else if let Some(parent) = entry.parent {
                by_parent.entry(parent).or_default().push(entry);
            }
        }

        /// Recursively builds a file tree.
        /// Builds the tree for `entry`, then removes an entry from `by_parent` using
        /// its id. If any entries were removed, calls itself again for each entry.
        fn build(
            entry: FileEntryTree,
            by_parent: &mut HashMap<Uuid, Vec<FileEntryTree>>,
        ) -> FileTree {
            let id = entry.id;

            let mut tree = FileTree::from(entry);

            if let Some(children) = by_parent.remove(&id) {
                tree.children = children
                    .into_iter()
                    .map(|child| build(child, by_parent))
                    .collect();
            }

            tree
        }

        roots
            .into_iter()
            .map(|entry| build(entry, &mut by_parent))
            .collect()
    }
}

impl From<FileEntryTree> for FileTree {
    fn from(value: FileEntryTree) -> Self {
        Self {
            id: value.id,
            name: value.name,
            kind: value.kind,
            file_type: value.file_type,
            metadata: value.metadata.into_option(),
            children: vec![],
        }
    }
}

#[derive(FromRow)]
pub struct FileEntryTree {
    pub id: Uuid,
    pub kind: FileKind,
    pub name: String,
    pub parent: Option<Uuid>,
    pub file_type: Option<FileType>,
    pub hash: Option<String>,
    #[sqlx(flatten)]
    pub metadata: FileMetadata,
    pub query_root: bool,
}

#[cfg(test)]
mod tests {
    use super::{FileEntryTree, FileTree};
    use crate::file::model::{FileKind, FileMetadata};
    use uuid::Uuid;

    fn entry(
        id: Uuid,
        name: &str,
        parent: Option<Uuid>,
        kind: FileKind,
        query_root: bool,
    ) -> FileEntryTree {
        FileEntryTree {
            id,
            kind,
            name: name.into(),
            parent,
            file_type: None,
            hash: None,
            metadata: FileMetadata::default(),
            query_root,
        }
    }

    fn names(trees: &[FileTree]) -> Vec<&str> {
        trees.iter().map(|tree| tree.name.as_str()).collect()
    }

    #[test]
    fn empty_input_has_no_roots() {
        assert!(FileTree::from_entries(vec![]).is_empty());
    }

    #[test]
    fn query_roots_keep_input_order() {
        let later = Uuid::new_v4();
        let earlier = Uuid::new_v4();
        let trees = FileTree::from_entries(vec![
            entry(later, "z", None, FileKind::File, true),
            entry(earlier, "a", None, FileKind::Directory, true),
        ]);

        assert_eq!(names(&trees), ["z", "a"]);
        assert!(trees.iter().all(|tree| tree.children.is_empty()));
    }

    #[test]
    fn children_attach_in_input_order_even_when_listed_first() {
        let parent = Uuid::new_v4();
        let file = Uuid::new_v4();
        let directory = Uuid::new_v4();
        let trees = FileTree::from_entries(vec![
            entry(file, "b.txt", Some(parent), FileKind::File, false),
            entry(directory, "a", Some(parent), FileKind::Directory, false),
            entry(
                parent,
                "notes",
                Some(Uuid::new_v4()),
                FileKind::Directory,
                true,
            ),
        ]);

        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].id, parent);
        assert_eq!(names(&trees[0].children), ["b.txt", "a"]);
    }

    #[test]
    fn grandchildren_nest_under_their_parent() {
        let notes = Uuid::new_v4();
        let nested = Uuid::new_v4();
        let leaf = Uuid::new_v4();
        let trees = FileTree::from_entries(vec![
            entry(notes, "notes", None, FileKind::Directory, true),
            entry(nested, "nested", Some(notes), FileKind::Directory, false),
            entry(leaf, "leaf.txt", Some(nested), FileKind::File, false),
        ]);

        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].children.len(), 1);
        assert_eq!(trees[0].children[0].id, nested);
        assert_eq!(trees[0].children[0].children.len(), 1);
        assert_eq!(trees[0].children[0].children[0].id, leaf);
        assert!(trees[0].children[0].children[0].children.is_empty());
    }

    #[test]
    fn rows_outside_the_query_roots_are_left_out() {
        let parent = Uuid::new_v4();
        let trees = FileTree::from_entries(vec![
            entry(
                Uuid::new_v4(),
                "orphan",
                Some(parent),
                FileKind::File,
                false,
            ),
            entry(Uuid::new_v4(), "loose", None, FileKind::File, false),
        ]);

        assert!(trees.is_empty());
    }
}
