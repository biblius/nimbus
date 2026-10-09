use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Database, Decode, Encode, Type, encode::IsNull, error::BoxDynError, prelude::FromRow};
use utoipa::ToSchema;
use uuid::Uuid;
use validify::Validify;

use super::format::FileFormat;
use crate::{db::Db, error::AppError, file::format::FileFamily};

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

/// A private or shared root folder.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub id: Uuid,
    pub name: String,
    /// `0` means usage of this folder for the user is unlimited.
    pub quota: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Access to a root; An assignment in the user_folders and its corresponding root.
/// If a user folder is `primary` it represents the user's private space.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RootView {
    pub id: Uuid,
    pub name: String,
    pub primary: bool,
    /// `0` means no quota.
    pub quota: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Data for creating a new [UserFolder].
pub struct RootAssignment {
    pub root_id: Uuid,
    pub user_id: Uuid,
    pub primary: bool,
}

/// Application native model. `parent` is null for entries that sit directly in a root.
#[derive(Debug, Clone, Serialize, ToSchema, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: Uuid,
    pub root_id: Uuid,
    pub created_by: Uuid,
    pub kind: FileKind,
    pub name: String,
    pub parent: Option<Uuid>,
    pub family: Option<FileFamily>,
    pub format: Option<FileFormat>,
    pub hash: Option<String>,
    pub size_bytes: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
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

#[derive(Debug, Deserialize, Validify, ToSchema)]
pub struct CreateDirectory {
    pub parent: Option<Uuid>,

    #[modify(trim)]
    #[validate(length(min = 1, message = "Directory name cannot be empty"))]
    #[validate(length(min = 255, message = "Directory too long"))]
    pub name: String,
}
