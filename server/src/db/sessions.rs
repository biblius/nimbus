use chrono::{DateTime, Utc};
use sqlx::Executor;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    auth::session::SessionRecord,
    db::{Db, DbPool},
    error::AppError,
};

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

    #[instrument(skip(self, session), fields(user_id = %session.user_id), err(Debug))]
    pub async fn insert(&self, session: NewSession) -> Result<SessionRecord, AppError> {
        self.insert_with(&self.pool, session).await
    }

    #[instrument(skip(self, executor, session), fields(user_id = %session.user_id), err(Debug))]
    pub async fn insert_with<'e, E>(
        &self,
        executor: E,
        session: NewSession,
    ) -> Result<SessionRecord, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
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
                id AS "id: Uuid",
                user_id AS "user_id: Uuid",
                remote_address,
                user_agent,
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>",
                expires_at AS "expires_at: DateTime<Utc>"
            "#,
            session.id,
            session.user_id,
            session.remote_address,
            session.user_agent,
            session.now,
            session.expires_at,
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip_all, err(Debug))]
    pub async fn get(&self, id: Uuid) -> Result<Option<SessionRecord>, AppError> {
        sqlx::query_as!(
            SessionRecord,
            r#"
            SELECT
                id AS "id: Uuid",
                user_id AS "user_id: Uuid",
                remote_address,
                user_agent,
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>",
                expires_at AS "expires_at: DateTime<Utc>"
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
    pub async fn list_live_for_user(&self, user_id: Uuid) -> Result<Vec<SessionRecord>, AppError> {
        let now = Utc::now();

        sqlx::query_as!(
            SessionRecord,
            r#"
            SELECT
                id AS "id: Uuid",
                user_id AS "user_id: Uuid",
                remote_address,
                user_agent,
                created_at AS "created_at: DateTime<Utc>",
                updated_at AS "updated_at: DateTime<Utc>",
                expires_at AS "expires_at: DateTime<Utc>"
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
            tracing::warn!("attempted to expire session that does not exist");
        }

        Ok(())
    }

    /// Hard-delete a session row (used when rotating the credential).
    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_with<'e, E>(&self, executor: E, id: Uuid) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let result = sqlx::query!(r#"DELETE FROM sessions WHERE id = $1"#, id)
            .execute(executor)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("Session does not exist".into()));
        }

        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn expire_all(&self, user_id: Uuid) -> Result<(), AppError> {
        self.expire_all_with(&self.pool, user_id).await
    }

    #[instrument(skip(self, executor), fields(user_id = %user_id), err(Debug))]
    pub async fn expire_all_with<'e, E>(&self, executor: E, user_id: Uuid) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
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
        .execute(executor)
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
