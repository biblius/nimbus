use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::user::model::User;

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
    pub expires_at: Option<DateTime<Utc>>,
    pub user: User,
}

impl Session {
    pub fn from_record(record: SessionRecord, user: User) -> Self {
        Self {
            id: record.id,
            expires_at: record.expires_at,
            user,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| expires_at < Utc::now())
    }
}

/// A live session row for the account settings list (no nested user).
#[derive(Debug, Clone, Serialize)]
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
