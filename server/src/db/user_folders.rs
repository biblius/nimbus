use chrono::{DateTime, Utc};
use sqlx::{Executor, QueryBuilder};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    db::{Db, DbPool},
    error::AppError,
    file::{
        access::AccessMode,
        model::{Root, RootAssignment},
    },
    model::{List, PaginationSort, ToSearchColumn},
    user::model::{Member, Role, UserSearch, UserSort},
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

    #[instrument(skip(self, user_ids), fields(root_id = %root_id, n = user_ids.len()), err(Debug))]
    pub async fn assign_many(&self, root_id: Uuid, user_ids: &[Uuid]) -> Result<(), AppError> {
        self.insert_many(&self.pool, root_id, user_ids).await
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
                user_id,
                access_mode
            )
            VALUES ($1, $2, $3)
            ON CONFLICT DO NOTHING
            "#,
            assignment.root_id,
            assignment.user_id,
            assignment.access_mode as _,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    /// Assign many users to a root in one statement. Unknown user ids fail the FK.
    ///
    /// New rows get [`AccessMode::DEFAULT`].
    #[instrument(skip(self, executor, user_ids), fields(root_id = %root_id, n = user_ids.len()), err(Debug))]
    pub async fn insert_many<'e, E>(
        &self,
        executor: E,
        root_id: Uuid,
        user_ids: &[Uuid],
    ) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        if user_ids.is_empty() {
            return Ok(());
        }

        let access_mode = AccessMode::DEFAULT;

        let mut query =
            QueryBuilder::<Db>::new("INSERT INTO user_folders (root_id, user_id, access_mode) ");

        query.push_values(user_ids, |mut row, user_id| {
            row.push_bind(root_id)
                .push_bind(*user_id)
                .push_bind(access_mode);
        });

        query.push(" ON CONFLICT DO NOTHING");

        query.build().execute(executor).await?;

        Ok(())
    }

    /// Load a root and the caller's membership [`AccessMode`], if any.
    ///
    /// `None` for the mode means the user is not in `user_folders` (private owner
    /// or no access). The service layer maps owners/admins to [`AccessMode::FULL`].
    #[instrument(skip(self), err(Debug))]
    pub async fn access(
        &self,
        user_id: Uuid,
        root_id: Uuid,
    ) -> Result<Option<(Root, Option<AccessMode>)>, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT
                r.id AS "id: Uuid",
                r.name,
                r.quota_bytes AS quota,
                r.owner AS "owner?: Uuid",
                r.created_at AS "created_at: DateTime<Utc>",
                r.updated_at AS "updated_at: DateTime<Utc>",
                uf.access_mode AS "access_mode?: AccessMode"
            FROM roots r
            LEFT JOIN user_folders uf
                ON uf.root_id = r.id AND uf.user_id = $2
            WHERE r.id = $1
            "#,
            root_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            (
                Root {
                    id: row.id,
                    name: row.name,
                    quota: row.quota,
                    owner: row.owner,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
                row.access_mode,
            )
        }))
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
    pub async fn list_users_in_paginated(
        &self,
        root_id: Uuid,
        pagination: PaginationSort<UserSort, UserSearch>,
    ) -> Result<List<Member>, AppError> {
        let (limit, offset) = pagination.limit_offset();
        let (sort_by, sort_dir) = pagination.sort();

        let mut q = QueryBuilder::new(
            r#"
            SELECT
                u.id,
                u.username,
                u.email,
                u.role,
                u.created_at
            FROM user_folders uf
            JOIN users u ON uf.user_id = u.id
            WHERE root_id = 
            "#,
        );

        q.push_bind(root_id);

        let mut count_q = QueryBuilder::new(
            r#"
            SELECT COUNT(*)
            FROM user_folders uf
            JOIN users u ON uf.user_id = u.id
            WHERE root_id = 
            "#,
        );

        count_q.push_bind(root_id);

        if let Some(ref search) = pagination.search {
            let pattern = format!("%{}%", search.q.to_lowercase());

            q.push(" AND LOWER(")
                .push(search.column.to_search_column_prefixed("u"))
                .push(") LIKE ")
                .push_bind(pattern.clone());

            count_q
                .push(" AND LOWER(")
                .push(search.column.to_search_column_prefixed("u"))
                .push(") LIKE ")
                .push_bind(pattern);
        }

        q.push(" ORDER BY ")
            .push(sort_by)
            .push(" ")
            .push(sort_dir)
            .push(" LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);

        let members = q
            .build_query_as::<Member>()
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        let count = count_q
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(List::new(count as usize, members))
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn list_users_in(&self, root_id: Uuid) -> Result<Vec<Member>, AppError> {
        sqlx::query_as!(
            Member,
            r#"
            SELECT
                u.id AS "id: Uuid",
                u.username,
                u.email,
                u.role AS "role: Role",
                u.created_at AS "created_at: DateTime<Utc>"
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

    /// Drop many memberships in one statement.
    #[instrument(skip(self, user_ids), fields(root_id = %root_id, n = user_ids.len()), err(Debug))]
    pub async fn remove_many(&self, root_id: Uuid, user_ids: &[Uuid]) -> Result<(), AppError> {
        if user_ids.is_empty() {
            return Ok(());
        }

        let mut query = QueryBuilder::<Db>::new("DELETE FROM user_folders WHERE root_id = ");
        query.push_bind(root_id).push(" AND user_id IN (");

        {
            let mut selected = query.separated(", ");
            for user_id in user_ids {
                selected.push_bind(*user_id);
            }
        }

        query.push(")");
        query.build().execute(&self.pool).await?;

        Ok(())
    }
}
