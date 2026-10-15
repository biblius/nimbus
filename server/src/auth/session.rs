use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::user::model::{User, UserViewFull};

#[derive(Debug, Clone, FromRow)]
pub struct SessionRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub remote_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// An authenticated session with the full user row. Internal only; never serialize it.
/// The id is the cookie value and the bearer token.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub remote_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub first_login: bool,
    pub user: User,
}

impl Session {
    pub fn from_record(record: SessionRecord, user: User, first_login: bool) -> Self {
        Self {
            id: record.id,
            remote_address: record.remote_address,
            user_agent: record.user_agent,
            created_at: record.created_at,
            updated_at: record.updated_at,
            expires_at: record.expires_at,
            first_login,
            user,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| expires_at < Utc::now())
    }
}

/// Client-facing session.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: Uuid,
    pub remote_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub first_login: bool,
    pub user: UserViewFull,
}

impl From<Session> for SessionView {
    fn from(session: Session) -> Self {
        Self {
            id: session.id,
            remote_address: session.remote_address,
            user_agent: session.user_agent,
            created_at: session.created_at,
            updated_at: session.updated_at,
            expires_at: session.expires_at,
            first_login: session.first_login,
            user: session.user.into(),
        }
    }
}

/// A live session row for the account settings list (no nested user).
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSessionView {
    pub id: Uuid,
    pub remote_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    /// True when this row is the caller's current session.
    pub current: bool,
}

impl ActiveSessionView {
    pub fn from_record(record: SessionRecord, current_id: Uuid) -> Self {
        Self {
            id: record.id,
            remote_address: record.remote_address,
            user_agent: record.user_agent,
            created_at: record.created_at,
            updated_at: record.updated_at,
            expires_at: record.expires_at,
            current: record.id == current_id,
        }
    }
}
