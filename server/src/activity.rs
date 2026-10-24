pub mod queue;

use crate::file::model::FileEntry;
use chrono::{DateTime, Utc};
pub use queue::{ActivityQueue, ActivityQueueHandle};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A user taken action in a root.
///
/// These are not meant to provide references to the entities acted upon
/// and is just here to capture user activity on a best-effort basis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub root_id: Uuid,
    pub username: String,
    pub data: ActivityData,
    pub created_at: DateTime<Utc>,
}

impl Activity {
    pub fn new(root_id: Uuid, username: String, data: ActivityData) -> Self {
        Self {
            root_id,
            username,
            data,
            created_at: Utc::now(),
        }
    }
}

/// Target of an activity:
///
/// - Single if the activity was only for 1 file, we can store the relation and name
///   if the relation ever gets removed.
/// - Multiple if the activity was for multiple files, skip storing relations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t_type", rename_all = "camelCase")]
pub enum ActivityTarget {
    Single {
        /// The file ID.
        id: Uuid,

        /// The file name for easy displaying and if the original is gone.
        name: String,
    },
    /// Stores the count of files acted upon.
    Multiple { count: usize },
}

impl From<FileEntry> for ActivityTarget {
    fn from(value: FileEntry) -> Self {
        Self::Single {
            id: value.id,
            name: value.name,
        }
    }
}

/// Payload stored in `root_activity.data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ActivityData {
    Create(ActivityTarget),
    Trash(ActivityTarget),
    Share(ActivityTarget),
    Move {
        target: ActivityTarget,
        /// Destination directory name
        to: String,
    },
    /// Only a single file can be renamed at once.
    Rename {
        /// File ID
        id: Uuid,
        /// Old file name
        from: String,
        /// New file name
        to: String,
    },
    Restore {
        target: ActivityTarget,
        /// Current name of the target restore directory
        to: String,
    },
    /// Only a single file can be edited at once.
    Edit {
        /// File ID
        id: Uuid,
        /// File name
        name: String,
    },
}
