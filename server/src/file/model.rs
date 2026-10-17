use std::path::PathBuf;

use super::format::FileType;
use crate::{db::Db, error::AppError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{
    Database, Decode, Encode, Type, encode::IsNull, error::BoxDynError, prelude::FromRow,
    types::Json,
};
use utoipa::ToSchema;
use uuid::Uuid;
use validify::{Validate, Validify};

/// Safety cap for upward parent walks.
///
/// The app never creates cyclic trees (moves reject any destination that is
/// the source or a descendant of it). This bound only guards the recursive
/// CTE against corrupt data or a bug.
pub(crate) const MAX_ANCESTOR_DEPTH: i64 = 1024;

/// Application native model. `parent` is null for entries that sit directly in a root.
/// Corresponds to the `files` / `trashed_files` SQL rows.
#[derive(Debug, Clone, Serialize, ToSchema, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: Uuid,
    pub root_id: Uuid,
    pub created_by: Option<Uuid>,
    pub kind: FileKind,
    pub name: String,
    pub parent: Option<Uuid>,
    pub file_type: Option<FileType>,
    pub hash: Option<String>,
    pub size_bytes: i64,
    /// When true, the entry is reachable via the public share routes.
    pub shared: bool,
    /// Generated artifacts (preview hashes, etc.). Omitted from JSON when null.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<FileMetadata>)]
    pub metadata: Option<Json<FileMetadata>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Generated preview hashes stored in the `metadata` JSONB column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    /// Content hash of the 64×64 WebP preview blob.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_64: Option<String>,
    /// Content hash of the 256×256 WebP preview blob.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_256: Option<String>,
}

/// Square preview edge length requested by clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewSize {
    /// List / small-tile icon.
    Icon = 64,
    /// Large-tile thumbnail.
    Tile = 256,
}

impl PreviewSize {
    pub fn from_px(px: u32) -> Result<Self, AppError> {
        match px {
            64 => Ok(Self::Icon),
            256 => Ok(Self::Tile),
            _ => Err(AppError::Unprocessable(
                "Preview size must be 64 or 256".into(),
            )),
        }
    }
}

/// A file entry with metadata lifted out for client listings.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    pub file: FileEntry,
    pub metadata: Option<FileMetadata>,
}

impl From<FileEntry> for FileView {
    fn from(mut entry: FileEntry) -> Self {
        let metadata = entry.metadata.take().map(|json| json.0);
        Self {
            file: entry,
            metadata,
        }
    }
}

impl FileEntry {
    pub fn valid_name(name: &str) -> Result<&str, AppError> {
        let name = name.trim();

        if name.is_empty() || name.len() > 255 {
            return Err(AppError::Unprocessable(
                "File name must be between 1 and 255 bytes".into(),
            ));
        }

        if name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
            || name.chars().any(|character| character.is_control())
        {
            return Err(AppError::Unprocessable(
                "File name contains invalid characters".into(),
            ));
        }

        Ok(name)
    }
}

/// A file id used when submitting hard deletes.
#[derive(Debug, Clone, Copy)]
pub struct FileRef {
    pub id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    File,
    Directory,
}

impl FileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "f",
            Self::Directory => "d",
        }
    }
}

impl Type<Db> for FileKind {
    fn type_info() -> <Db as Database>::TypeInfo {
        <str as Type<Db>>::type_info()
    }

    fn compatible(ty: &<Db as Database>::TypeInfo) -> bool {
        <str as Type<Db>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Db> for FileKind {
    fn encode_by_ref(
        &self,
        buf: &mut <Db as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <&'q str as Encode<'q, Db>>::encode_by_ref(&self.as_str(), buf)
    }
}

impl<'r> Decode<'r, Db> for FileKind {
    fn decode(value: <Db as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        match <&'r str as Decode<'r, Db>>::decode(value)? {
            "f" => Ok(Self::File),
            "d" => Ok(Self::Directory),
            other => Err(format!("unknown file kind: {other:?}").into()),
        }
    }
}

/// A private or shared root folder. `owner` is set only for a private root.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub id: Uuid,
    pub name: String,
    /// `0` means usage of this folder for the user is unlimited.
    pub quota: i64,
    pub owner: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A root as returned to a client. `owner` is set only for a private root.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RootView {
    pub id: Uuid,
    pub name: String,
    /// `0` means no quota.
    pub quota: i64,
    pub owner: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Membership of a shared root.
pub struct RootAssignment {
    pub root_id: Uuid,
    pub user_id: Uuid,
}

/// Where a trashed item should be put back. If `parent` is null, the file is stored directly in root.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestoreFile {
    pub parent: Option<Uuid>,
}

/// Files to move to the trash in one request.
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct TrashFiles {
    #[validate(length(min = 1, message = "At least 1 file must be provided"))]
    pub ids: Vec<Uuid>,
}

/// Live files whose parent should change. `parent` is null to place them in the root.
#[derive(Debug, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct MoveFiles {
    #[validate(length(min = 1, message = "At least 1 file must be provided"))]
    pub ids: Vec<Uuid>,
    pub parent: Option<Uuid>,
}

/// A live file (or directory tree) to copy into another accessible root.
/// `parent` is null to place the copy directly in the target root.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CopyFile {
    pub id: Uuid,
    pub target_root_id: Uuid,
    pub parent: Option<Uuid>,
}

/// New display name for a live file or directory.
#[derive(Debug, Deserialize, Validify, ToSchema)]
pub struct RenameFile {
    #[modify(trim)]
    #[validate(length(min = 1, message = "File name cannot be empty"))]
    #[validate(length(max = 255, message = "File name too long"))]
    pub name: String,
}

#[derive(Debug, Deserialize, Validify, ToSchema)]
pub struct CreateDirectory {
    /// Used internally, clients do not send this.
    #[serde(skip)]
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[modify(trim)]
    #[validate(length(min = 1, message = "Directory name cannot be empty"))]
    #[validate(length(max = 255, message = "Directory name too long"))]
    pub name: String,

    pub parent: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validify, ToSchema)]
pub struct CreateTextFile {
    #[modify(trim)]
    #[validate(length(min = 1, message = "File name cannot be empty"))]
    #[validate(length(max = 255, message = "File name too long"))]
    pub name: String,

    pub parent: Option<Uuid>,
}

impl CreateDirectory {
    pub fn new(id: Uuid, name: String, parent: Option<Uuid>) -> Self {
        Self { id, parent, name }
    }
}

impl Default for CreateDirectory {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            parent: None,
        }
    }
}

pub struct FileContentPreview {
    /// File handle.
    pub file: tokio::fs::File,
    /// Content type.
    pub mime: &'static str,
    /// Size of the file in bytes.
    pub len: u64,
}

impl FileContentPreview {
    pub async fn new(path: PathBuf, mime: &'static str) -> Result<Self, AppError> {
        let file = tokio::fs::File::open(&path).await?;
        let len = tokio::fs::metadata(&path).await?.len();

        Ok(Self { file, mime, len })
    }
}

/// Works as a command for user initiated deletion requests,
/// indicating what to do after file deletion completes.
#[derive(Debug)]
pub enum Cleanup {
    /// Remove the user and their root (for deleting users).
    User { user_id: Uuid, root_id: Uuid },
    /// Remove the root (for shared roots).
    Root { root_id: Uuid },
}
