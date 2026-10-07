use std::fmt::Display;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Database, Decode, Encode, Type, encode::IsNull, error::BoxDynError};
use utoipa::ToSchema;
use uuid::Uuid;
use validify::{Validate, ValidationError, Validify, field_err};

use crate::db::Db;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    #[default]
    User,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Type<Db> for Role {
    fn type_info() -> <Db as Database>::TypeInfo {
        <str as Type<Db>>::type_info()
    }

    fn compatible(ty: &<Db as Database>::TypeInfo) -> bool {
        <str as Type<Db>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Db> for Role {
    fn encode_by_ref(
        &self,
        buf: &mut <Db as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <&'q str as Encode<'q, Db>>::encode_by_ref(&self.as_str(), buf)
    }
}

impl<'r> Decode<'r, Db> for Role {
    fn decode(value: <Db as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        match <&'r str as Decode<'r, Db>>::decode(value)? {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            other => Err(format!("unknown role stored in the database: {other:?}").into()),
        }
    }
}

/// User row, including secrets. Never serialize this to a client.
#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub role: Role,
    pub otp_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// The view of the currently logged in user.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserViewFull {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub role: Role,
    pub otp_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserViewFull {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            username: value.username,
            email: value.email,
            role: value.role,
            otp_enabled: value.otp_secret.is_some(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

/// The view for users that are not the current one.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserView {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub role: Role,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserView {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            username: value.username,
            email: value.email,
            role: value.role,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Deserialize, Validify, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateUser {
    #[modify(trim, lowercase)]
    #[validate(length(min = 1, max = 64))]
    #[validate(custom(function = valid_username))]
    pub username: String,

    #[validate(length(min = 1, max = 256))]
    pub password: String,

    #[modify(trim, lowercase)]
    #[validate(email)]
    pub email: Option<String>,

    #[serde(default)]
    pub role: Role,
}

/// Fields an admin can change on an existing account.
/// Omit `password` to keep the current one.
#[derive(Debug, Deserialize, Validify, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUser {
    #[modify(trim, lowercase)]
    #[validate(length(min = 1, max = 64))]
    #[validate(custom(function = valid_username))]
    pub username: Option<String>,

    #[modify(trim, lowercase)]
    #[validate(email)]
    pub email: Option<String>,

    pub role: Role,

    #[serde(default)]
    #[validate(length(min = 1, max = 256))]
    pub password: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetQuota {
    /// `0` means the folder is unlimited.
    #[validate(range(min = 0.))]
    pub quota_bytes: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FolderQuota {
    pub root_id: Uuid,
    pub quota_bytes: i64,
}

#[derive(Debug, Deserialize, Validify, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedFolder {
    #[modify(trim)]
    #[validate(length(min = 1, max = 255))]
    pub name: String,

    /// `0` means the shared folder is unlimited.
    #[validate(range(min = 0.))]
    pub quota_bytes: i64,

    #[serde(default)]
    pub members: Vec<Uuid>,
}

/// Name and quota of an existing root.
#[derive(Debug, Deserialize, Validify, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoot {
    #[modify(trim)]
    #[validate(length(min = 1, max = 255))]
    pub name: String,

    /// `0` means the folder is unlimited.
    #[validate(range(min = 0.))]
    pub quota_bytes: i64,
}

fn valid_username(username: &str) -> Result<(), ValidationError> {
    if username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
    {
        return Ok(());
    }
    Err(field_err!("username"))
}
