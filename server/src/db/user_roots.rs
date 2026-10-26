use chrono::{DateTime, Utc};
use sqlx::{Executor, FromRow, QueryBuilder};
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
    user::model::{AddMember, Member, UserSearch, UserSort},
};

#[derive(FromRow)]
struct RootAccessRow {
    id: Uuid,
    name: String,
    quota: i64,
    owner: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    access_mode: Option<AccessMode>,
}

#[derive(Clone)]
pub struct UserRootRepository {
    pool: DbPool,
}

impl UserRootRepository {
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
        sqlx::query(
            r#"
            INSERT INTO user_roots (
                root_id,
                user_id,
                access_mode
            )
            VALUES ($1, $2, $3)
            ON CONFLICT (root_id, user_id) DO UPDATE SET
                access_mode = EXCLUDED.access_mode
            "#,
        )
        .bind(assignment.root_id)
        .bind(assignment.user_id)
        .bind(assignment.access_mode)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// Upsert many memberships. Unknown user ids fail the FK.
    ///
    /// Existing rows get their `access_mode` replaced.
    #[instrument(skip(self, executor, members), fields(root_id = %root_id, n = members.len()), err(Debug))]
    pub async fn insert_many<'e, E>(
        &self,
        executor: E,
        root_id: Uuid,
        members: &[AddMember],
    ) -> Result<(), AppError>
    where
        E: Executor<'e, Database = Db>,
    {
        if members.is_empty() {
            return Ok(());
        }

        let mut query =
            QueryBuilder::<Db>::new("INSERT INTO user_roots (root_id, user_id, access_mode) ");

        query.push_values(members, |mut row, member| {
            row.push_bind(root_id)
                .push_bind(member.user_id)
                .push_bind(member.access_mode);
        });

        query.push(
            " ON CONFLICT (root_id, user_id) DO UPDATE SET access_mode = EXCLUDED.access_mode",
        );

        query.build().execute(executor).await?;

        Ok(())
    }

    /// Load a root and the caller's membership [`AccessMode`], if any.
    ///
    /// `None` for the mode means the user is not in `user_roots` (private owner
    /// or no access). The service layer maps owners/admins to [`AccessMode::FULL`].
    #[instrument(skip(self), err(Debug))]
    pub async fn access(
        &self,
        user_id: Uuid,
        root_id: Uuid,
    ) -> Result<Option<(Root, Option<AccessMode>)>, AppError> {
        let row = sqlx::query_as::<_, RootAccessRow>(
            r#"
            SELECT
                r.id,
                r.name,
                r.quota_bytes AS quota,
                r.owner,
                r.created_at,
                r.updated_at,
                uf.access_mode
            FROM roots r
            LEFT JOIN user_roots uf
                ON uf.root_id = r.id AND uf.user_id = $2
            WHERE r.id = $1
            "#,
        )
        .bind(root_id)
        .bind(user_id)
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
        let found = sqlx::query_scalar::<_, i32>(
            r#"
            SELECT 1
            FROM roots
            WHERE id = $1
            AND (
                owner = $2
                OR EXISTS (
                    SELECT 1
                    FROM user_roots
                    WHERE root_id = roots.id AND user_id = $2
                )
            )
            "#,
        )
        .bind(root_id)
        .bind(user_id)
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
                uf.access_mode,
                u.created_at
            FROM user_roots uf
            JOIN users u ON uf.user_id = u.id
            WHERE root_id = 
            "#,
        );

        q.push_bind(root_id);

        let mut count_q = QueryBuilder::new(
            r#"
            SELECT COUNT(*)
            FROM user_roots uf
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
        sqlx::query_as::<_, Member>(
            r#"
            SELECT
                u.id,
                u.username,
                u.email,
                u.role,
                uf.access_mode,
                u.created_at
            FROM user_roots uf
            JOIN users u ON uf.user_id = u.id
            WHERE root_id = $1
            ORDER BY username
            "#,
        )
        .bind(root_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn remove(&self, root_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM user_roots
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
    #[instrument(skip(self, executor, user_ids), fields(root_id = %root_id, n = user_ids.len()), err(Debug))]
    pub async fn remove_many<'e, E>(
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

        let mut query = QueryBuilder::<Db>::new("DELETE FROM user_roots WHERE root_id = ");
        query.push_bind(root_id).push(" AND user_id IN (");

        {
            let mut selected = query.separated(", ");
            for user_id in user_ids {
                selected.push_bind(*user_id);
            }
        }

        query.push(")");
        query.build().execute(executor).await?;

        Ok(())
    }
}
