use chrono::{DateTime, Utc};
use tracing::instrument;
use uuid::Uuid;

use crate::{auth::session::SessionRecord, db::DbPool, error::AppError};

#[derive(Clone)]
pub struct SessionRepository {
    pool: DbPool,
}

pub struct NewSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub remote_address: Option<String>,
    pub user_agent: Option<String>,
    pub now: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl SessionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn count_for_user(&self, user_id: Uuid) -> Result<i64, AppError> {
        sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!"
            FROM sessions
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self, session), fields(user_id = %session.user_id), err(Debug))]
    pub async fn insert(&self, session: NewSession) -> Result<SessionRecord, AppError> {
        sqlx::query_as!(
            SessionRecord,
            r#"
            INSERT INTO sessions (
                id,
                user_id,
                remote_address,
                user_agent,
                created_at,
                updated_at,
                expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $5, $6)
            RETURNING 
                id,
                user_id,
                remote_address,
                user_agent,
                created_at,
                updated_at,
                expires_at
            "#,
            session.id,
            session.user_id,
            session.remote_address,
            session.user_agent,
            session.now,
            session.expires_at,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip_all, err(Debug))]
    pub async fn get(&self, id: Uuid) -> Result<Option<SessionRecord>, AppError> {
        sqlx::query_as!(
            SessionRecord,
            r#"
            SELECT 
                id,
                user_id,
                remote_address,
                user_agent,
                created_at,
                updated_at,
                expires_at
            FROM sessions
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    /// Live (non-expired) sessions for a user, newest first.
    #[instrument(skip(self), err(Debug))]
    pub async fn list_live_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<SessionRecord>, AppError> {
        let now = Utc::now();

        sqlx::query_as!(
            SessionRecord,
            r#"
            SELECT
                id,
                user_id,
                remote_address,
                user_agent,
                created_at,
                updated_at,
                expires_at
            FROM sessions
            WHERE
                user_id = $1
            AND (expires_at IS NULL OR expires_at > $2)
            ORDER BY created_at DESC
            "#,
            user_id,
            now,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip_all, err(Debug))]
    pub async fn expire(&self, id: Uuid) -> Result<(), AppError> {
        let now = Utc::now();

        let result = sqlx::query!(
            r#"
            UPDATE sessions 
            SET 
                expires_at = $1,
                updated_at = $1 
            WHERE id = $2
            "#,
            now,
            id
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            tracing::warn!(%id, "attempted to expire session that does not exist");
        }

        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn expire_all(&self, user_id: Uuid) -> Result<(), AppError> {
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE sessions
            SET 
                expires_at = $1,
                updated_at = $1
            WHERE 
                user_id = $2 
            AND (expires_at IS NULL OR expires_at > $1)
            "#,
            now,
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[instrument(skip(self, keep), fields(user_id = %user_id), err(Debug))]
    pub async fn expire_others(&self, user_id: Uuid, keep: Uuid) -> Result<(), AppError> {
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE sessions
            SET 
                expires_at = $1,
                updated_at = $1
            WHERE 
                user_id = $2 
            AND id != $3 
            AND (expires_at IS NULL OR expires_at > $1)
            "#,
            now,
            user_id,
            keep,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
