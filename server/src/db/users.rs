use chrono::Utc;
use sqlx::{Executor, QueryBuilder};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    auth::model::UserPasskey,
    db::{Db, DbPool},
    error::AppError,
    model::{List, PaginationSort, ToSearchColumn},
    user::model::{CreateUser, Role, User, UserSearch, UserSort, UserView},
};
use sqlx::types::Json;
use webauthn_rs::prelude::{CredentialID, Passkey};

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
    pub async fn list_paginated(
        &self,
        pagination: PaginationSort<UserSort, UserSearch>,
    ) -> Result<List<UserView>, AppError> {
        let (limit, offset) = pagination.limit_offset();
        let (sort_by, sort_dir) = pagination.sort();

        let mut listed = QueryBuilder::<Db>::new(
            r#"
            SELECT
                id,
                username,
                email,
                role,
                otp_secret IS NOT NULL AS otp_enabled,
                created_at,
                updated_at
            FROM users
            "#,
        );
        let mut count = QueryBuilder::<Db>::new("SELECT COUNT(*) FROM users");

        if let Some(ref search) = pagination.search {
            let column = search.column.to_search_column();

            listed
                .push(" WHERE ")
                .push(column)
                .push(" ILIKE '%' || ")
                .push_bind(&search.q)
                .push(" || '%'");

            count
                .push(" WHERE ")
                .push(column)
                .push(" ILIKE '%' || ")
                .push_bind(&search.q)
                .push(" || '%'");
        }

        listed
            .push(" ORDER BY ")
            .push(sort_by)
            .push(" ")
            .push(sort_dir)
            .push(" LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);

        let items = listed
            .build_query_as::<UserView>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        let total = count
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(List::new(total as usize, items))
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
            User,
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
                password AS "password!",
                email,
                role AS "role: Role",
                otp_secret,
                created_at,
                updated_at
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

        user.map(UserView::from)
            .ok_or_else(|| AppError::ResourceMissing("User does not exist".into()))
    }

    #[instrument(skip(self, password), fields(user_id = %id), err(Debug))]
    pub async fn update_password(&self, id: Uuid, password: &str) -> Result<(), AppError> {
        self.update_password_with(&self.pool, id, password).await
    }

    #[instrument(skip(self, executor, password), fields(user_id = %id), err(Debug))]
    pub async fn update_password_with<'e, E>(
        &self,
        executor: E,
        id: Uuid,
        password: &str,
    ) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
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
        .execute(executor)
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

    #[instrument(skip(self), fields(user_id = %id), err(Debug))]
    pub async fn clear_otp_secret(&self, id: Uuid) -> Result<(), AppError> {
        self.clear_otp_secret_with(&self.pool, id).await
    }

    #[instrument(skip(self, executor), fields(user_id = %id), err(Debug))]
    pub async fn clear_otp_secret_with<'e, E>(&self, executor: E, id: Uuid) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        let now = Utc::now();

        let result = sqlx::query!(
            r#"
            UPDATE users
            SET
                otp_secret = NULL,
                updated_at = $1
            WHERE
                id = $2
            "#,
            now,
            id
        )
        .execute(executor)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing("User does not exist".into()));
        }

        Ok(())
    }

    /// Delete every passkey registered to the user.
    #[instrument(skip(self, executor), fields(user_id = %user_id), err(Debug))]
    pub async fn clear_passkeys_with<'e, E>(
        &self,
        executor: E,
        user_id: Uuid,
    ) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query!(
            r#"
            DELETE FROM user_passkeys
            WHERE user_id = $1
            "#,
            user_id
        )
        .execute(executor)
        .await?;

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

    /// Whether the user has at least one registered passkey.
    #[instrument(skip(self), fields(user_id = %user_id), err(Debug))]
    pub async fn is_passkey_enabled(&self, user_id: Uuid) -> Result<bool, AppError> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM user_passkeys WHERE user_id = $1
            ) AS "exists!"
            "#,
            user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(exists)
    }

    #[instrument(skip(self), fields(user_id = %user_id), err(Debug))]
    pub async fn list_passkeys(&self, user_id: Uuid) -> Result<Vec<UserPasskey>, AppError> {
        sqlx::query_as!(
            UserPasskey,
            r#"
            SELECT
                id,
                user_id,
                data AS "data: Json<Passkey>",
                created_at
            FROM user_passkeys
            WHERE user_id = $1
            ORDER BY created_at
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self, passkey), fields(user_id = %user_id), err(Debug))]
    pub async fn insert_passkey(
        &self,
        user_id: Uuid,
        passkey: Passkey,
    ) -> Result<UserPasskey, AppError> {
        let now = Utc::now();
        let data = Json(passkey);

        sqlx::query_as!(
            UserPasskey,
            r#"
            INSERT INTO user_passkeys (user_id, data, created_at)
            VALUES ($1, $2, $3)
            RETURNING
                id,
                user_id,
                data AS "data: Json<Passkey>",
                created_at
            "#,
            user_id,
            data as Json<Passkey>,
            now
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    /// Whether any account already owns this authenticator credential.
    #[instrument(skip(self, cred_id), err(Debug))]
    pub async fn passkey_cred_id_taken(&self, cred_id: &CredentialID) -> Result<bool, AppError> {
        let passkeys = sqlx::query_scalar!(
            r#"
            SELECT data AS "data: Json<Passkey>"
            FROM user_passkeys
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(passkeys.iter().any(|passkey| passkey.cred_id() == cred_id))
    }

    /// Persist counter / backup-state updates after a successful passkey authentication.
    #[instrument(skip(self, passkey), err(Debug))]
    pub async fn apply_passkey_authentication(
        &self,
        id: i64,
        passkey: Passkey,
    ) -> Result<(), AppError> {
        let data = Json(passkey);
        sqlx::query!(
            r#"
            UPDATE user_passkeys
            SET data = $1
            WHERE id = $2 
            "#,
            data as Json<Passkey>,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
