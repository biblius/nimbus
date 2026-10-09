use sqlx::Executor;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::model::RootAssignment,
    user::model::{Member, Role},
};

#[derive(Clone)]
pub struct UserFolderRepository {
    pool: DbPool,
}

impl UserFolderRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self, assignment), fields(root_id = %assignment.root_id, user_id = %assignment.user_id), err(Debug))]
    pub async fn assign(&self, assignment: RootAssignment) -> Result<(), AppError> {
        self.insert(&self.pool, assignment).await
    }

    #[instrument(skip(self, executor, assignment), fields(root_id = %assignment.root_id, user_id = %assignment.user_id), err(Debug))]
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
                user_id
            )
            VALUES ($1, $2)
            ON CONFLICT DO NOTHING
            "#,
            assignment.root_id,
            assignment.user_id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn can_access(&self, user_id: Uuid, root_id: Uuid) -> Result<bool, AppError> {
        let found = sqlx::query_scalar!(
            r#"
            SELECT 1
            FROM roots
            WHERE id = $1
            AND (
                owner = $2
                OR EXISTS (
                    SELECT 1
                    FROM user_folders
                    WHERE root_id = roots.id AND user_id = $2
                )
            )
            "#,
            root_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(found.is_some())
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_users_in(&self, root_id: Uuid) -> Result<Vec<Member>, AppError> {
        sqlx::query_as!(
            Member,
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

    #[instrument(skip(self), err(Debug))]
    pub async fn remove(&self, root_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM user_folders
            WHERE
                root_id = $1
            AND user_id = $2
            "#,
        )
        .bind(root_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::ResourceMissing(
                "That user is not assigned to this folder".into(),
            ));
        }

        Ok(())
    }
}
