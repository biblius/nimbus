use chrono::Utc;
use sqlx::Executor;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    db::{Db, DbPool},
    error::AppError,
    user::model::{CreateUser, Role, User, UserView, UserViewFull},
};

#[derive(Clone)]
pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn count(&self) -> Result<i64, AppError> {
        sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!"
            FROM users
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn count_admins(&self) -> Result<i64, AppError> {
        sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!"
            FROM users 
            WHERE role = $1"#,
            Role::Admin as _
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn exists(&self, username: &str) -> Result<bool, AppError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!"
            FROM users 
            WHERE username = $1"#,
            username
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn get_by_id(&self, id: Uuid) -> Result<User, AppError> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT
                id,
                username,
                password AS "password!",
                email,
                role AS "role: Role",
                otp_secret, 
                created_at,
                updated_at
            FROM users
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        match user {
            Some(user) => Ok(user),
            None => Err(AppError::ResourceMissing("User not found".into())),
        }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn get_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as!(
            User,
            r#"
            SELECT
                id,
                username,
                password AS "password!",
                email,
                role AS "role: Role",
                otp_secret, 
                created_at,
                updated_at
            FROM users
            WHERE username = $1
            "#,
            username
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn get_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as!(
            User,
            r#"
            SELECT
                id,
                username,
                password AS "password!",
                email,
                role AS "role: Role",
                otp_secret,
                created_at,
                updated_at
            FROM users
            WHERE email = $1
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list(&self) -> Result<Vec<UserViewFull>, AppError> {
        sqlx::query_as!(
            UserViewFull,
            r#"
            SELECT
                id,
                username,
                email,
                role AS "role: Role",
                otp_secret IS NOT NULL AS "otp_enabled!",
                created_at,
                updated_at
            FROM users
            ORDER BY username
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self, executor, user), fields(user_id = %id, username = %user.username), err(Debug))]
    pub async fn insert<'e, E>(
        &self,
        executor: E,
        id: Uuid,
        user: &CreateUser,
    ) -> Result<User, AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let now = Utc::now();

        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (
                id,
                username,
                password,
                email,
                role,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $6)
            RETURNING
                id,
                username,
                password AS "password!",
                email,
                role AS "role: Role",
                otp_secret,
                created_at,
                updated_at
            "#,
            id,
            user.username,
            user.password,
            user.email,
            user.role as _,
            now
        )
        .fetch_one(executor)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self, password), err(Debug))]
    pub async fn update(
        &self,
        id: Uuid,
        username: Option<&str>,
        email: Option<&str>,
        role: Role,
        password: Option<&str>,
    ) -> Result<UserView, AppError> {
        let now = Utc::now();

        let user = sqlx::query_as!(
            UserView,
            r#"
            UPDATE users
            SET
                username = COALESCE($1, username),
                email = $2,
                role = $3,
                password = COALESCE($4, password),
                updated_at = $5
            WHERE id = $6
            RETURNING
                id,
                username,
                email,
                role AS "role: Role",
                created_at
            "#,
            username,
            email,
            role as _,
            password,
            now,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        user.ok_or_else(|| AppError::ResourceMissing("User does not exist".into()))
    }

    #[instrument(skip(self, password), fields(user_id = %id), err(Debug))]
    pub async fn update_password(&self, id: Uuid, password: &str) -> Result<(), AppError> {
        let now = Utc::now();

        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET 
                password = $1,
                updated_at = $2 
            WHERE 
                id = $3"#,
            password,
            now,
            id
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("User does not exist".into()));
        }

        Ok(())
    }

    #[instrument(skip(self, secret), fields(user_id = %id), err(Debug))]
    pub async fn update_otp_secret(&self, id: Uuid, secret: &str) -> Result<(), AppError> {
        let now = Utc::now();

        let result = sqlx::query!(
            r#"
            UPDATE users 
            SET 
                otp_secret = $1,
                updated_at = $2 
            WHERE 
                id = $3
            "#,
            secret,
            now,
            id
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("User does not exist".into()));
        }

        Ok(())
    }

    #[instrument(skip(self, executor), err(Debug))]
    pub async fn delete_with<'e, E>(&self, executor: E, id: Uuid) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let result = sqlx::query!(r#"DELETE FROM users WHERE id = $1"#, id)
            .execute(executor)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("User does not exist".into()));
        }

        Ok(())
    }
}
