use sqlx::Executor;
use uuid::Uuid;

use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::model::{RootAssignment, RootView},
    user::model::{Role, UserView},
};

#[derive(Clone)]
pub struct UserFolderRepository {
    pool: DbPool,
}

impl UserFolderRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn assign(&self, assignment: RootAssignment) -> Result<(), AppError> {
        self.insert(&self.pool, assignment).await
    }

    pub async fn insert<'e, E>(
        &self,
        executor: E,
        assignment: RootAssignment,
    ) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        sqlx::query!(
            r#"
            INSERT INTO user_folders (
                root_id,
                user_id,
                "primary"
            )
            VALUES ($1, $2, $3)
            ON CONFLICT DO NOTHING
            "#,
            assignment.root_id,
            assignment.user_id,
            assignment.primary,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn list_roots(&self, user_id: Uuid) -> Result<Vec<RootView>, AppError> {
        sqlx::query_as!(
            RootView,
            r#"
            SELECT 
                r.id,
                r.name,
                r.quota_bytes AS quota,
                r.created_at,
                r.updated_at,
                uf."primary"
            FROM user_folders uf
            JOIN roots r ON r.id = uf.root_id
            WHERE user_id = $1
            ORDER BY "primary" DESC, root_id
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn can_access(&self, user_id: Uuid, root_id: Uuid) -> Result<bool, AppError> {
        let found = sqlx::query_scalar!(
            r#"
            SELECT 1
            FROM user_folders 
            WHERE 
                root_id = $1 
            AND user_id = $2
            "#,
            root_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(found.is_some())
    }

    pub async fn set_quota(&self, root_id: Uuid, quota_bytes: i64) -> Result<(), AppError> {
        let updated = sqlx::query(r#"UPDATE user_folders SET quota_bytes = $1 WHERE root_id = $2"#)
            .bind(quota_bytes)
            .bind(root_id)
            .execute(&self.pool)
            .await?
            .rows_affected();

        if updated == 0 {
            return Err(AppError::ResourceMissing(
                "folder assignment does not exist".into(),
            ));
        }

        Ok(())
    }

    pub async fn list_users_in(&self, root_id: Uuid) -> Result<Vec<UserView>, AppError> {
        sqlx::query_as!(
            UserView,
            r#"
            SELECT 
                u.id,
                u.username,
                u.email,
                u.role AS "role: Role",
                u.created_at
            FROM user_folders uf
            JOIN users u ON uf.user_id = u.id
            WHERE root_id = $1
            ORDER BY username
            "#,
            root_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn is_primary(&self, root_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        let found: Option<bool> = sqlx::query_scalar(
            r#"SELECT "primary" FROM user_folders WHERE root_id = $1 AND user_id = $2"#,
        )
        .bind(root_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(found.unwrap_or(false))
    }

    pub async fn remove(&self, root_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM user_folders
            WHERE
                root_id = $1
            AND user_id = $2
            AND "primary" = $3"#,
        )
        .bind(root_id)
        .bind(user_id)
        .bind(false)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing(
                "that user is not assigned to this folder".into(),
            ));
        }

        Ok(())
    }
}
