use std::fmt::Display;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validify::{Validate, ValidationError, Validify, field_err};

use crate::{search_column, sort_column};

/// User row, including secrets. Never serialize this to a client.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub role: Role,
    pub otp_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    fn valid_username(username: &str) -> Result<(), ValidationError> {
        if username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
        {
            return Ok(());
        }
        Err(field_err!(
            "username",
            "Username must consist of only letters, numbers, underscores (_), dots (.), or hyphens (-)"
        ))
    }
}

/// The view of the currently logged in user.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserView {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub role: Role,
    pub otp_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserView {
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
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

sort_column! {
    UserSort,
    Username => "username",
    Email => "email",
    CreatedAt => "created_at",
    UpdatedAt => "updated_at"
}

search_column! {
    UserSearch,
    Username => "username",
    Email => "email"
}

#[derive(Debug, Deserialize, Validify)]
#[serde(rename_all = "camelCase")]
pub struct CreateUser {
    #[validate(length(min = 1, message = "Username cannot be empty"))]
    #[validate(length(max = 63, message = "Username too long"))]
    #[validate(custom(function = User::valid_username))]
    pub username: String,

    #[validate(length(min = 1, message = "Password cannot be empty"))]
    #[validate(length(max = 255, message = "Password too long"))]
    pub password: String,

    #[modify(trim, lowercase)]
    #[validate(email(message = "Not a valid email"))]
    pub email: Option<String>,

    pub role: Role,
}

/// Fields an admin can change on an existing account.
/// Omit `password` to keep the current one.
#[derive(Debug, Deserialize, Validify)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUser {
    #[modify(trim, lowercase)]
    #[validate(length(min = 1, message = "Username cannot be empty"))]
    #[validate(length(max = 63, message = "Username too long"))]
    #[validate(custom(function = User::valid_username))]
    pub username: Option<String>,

    #[validate(length(min = 1, message = "Password cannot be empty"))]
    #[validate(length(max = 255, message = "Password too long"))]
    pub password: Option<String>,

    #[modify(trim, lowercase)]
    #[validate(email(message = "Not a valid email"))]
    pub email: Option<String>,

    pub role: Role,
}

/// Fields a signed-in user can change on their own account.
/// Omit `password` to keep the current one. Role cannot be changed here.
/// When setting `password`, `current_password` must match the existing one.
#[derive(Debug, Deserialize, Validify)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSelf {
    #[modify(trim, lowercase)]
    #[validate(length(min = 1, message = "Username cannot be empty"))]
    #[validate(length(max = 63, message = "Username too long"))]
    #[validate(custom(function = User::valid_username))]
    pub username: Option<String>,

    /// Required when `password` is set.
    #[validate(length(min = 1, message = "Current password cannot be empty"))]
    pub current_password: Option<String>,

    #[validate(length(min = 1, message = "Password cannot be empty"))]
    #[validate(length(max = 255, message = "Password too long"))]
    pub password: Option<String>,

    #[modify(trim, lowercase)]
    #[validate(email(message = "Not a valid email"))]
    pub email: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SetQuota {
    /// `0` means the folder is unlimited.
    #[validate(range(min = 0., message = "Quota must be 0 (unlimited) or higher"))]
    pub quota_bytes: i64,
}

/// One membership to create or keep on a shared root.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMember {
    pub user_id: Uuid,
    /// Bitflags: `WRITE = 1`, `SHARE = 2` (see [`crate::file::AccessMode`]).
    pub access_mode: crate::file::AccessMode,
}

#[cfg(test)]
impl From<Uuid> for AddMember {
    fn from(user_id: Uuid) -> Self {
        Self {
            user_id,
            access_mode: crate::file::AccessMode::DEFAULT,
        }
    }
}

#[derive(Debug, Deserialize, Validify)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedFolder {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    #[validate(length(max = 255, message = "Name too long"))]
    pub name: String,

    /// `0` means the shared folder is unlimited.
    #[validate(range(min = 0., message = "Quota must be 0 (unlimited) or higher"))]
    pub quota_bytes: i64,

    #[validate(length(min = 1, message = "At least one member must be provided"))]
    pub members: Vec<AddMember>,
}

/// Name, quota, and members of an existing root.
/// Members apply only to a shared root. A private root cannot gain members.
#[derive(Debug, Deserialize, Validify)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoot {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    #[validate(length(max = 255, message = "Name too long"))]
    pub name: String,

    /// `0` means the folder is unlimited.
    #[validate(range(min = 0., message = "Quota must be 0 (unlimited) or higher"))]
    pub quota_bytes: i64,

    #[serde(default)]
    pub members: Vec<AddMember>,
}

/// A user assigned to a shared root.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub role: Role,
    pub access_mode: crate::file::AccessMode,
    pub created_at: DateTime<Utc>,
}
