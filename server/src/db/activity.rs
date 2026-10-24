use crate::{
    activity::{Activity, ActivityData},
    db::DbPool,
    error::AppError,
    model::{List, Pagination},
};
use chrono::{DateTime, Utc};
use sqlx::types::Json;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct ActivityRepository {
    pool: DbPool,
}

#[derive(Debug, sqlx::FromRow)]
struct ActivityRow {
    root_id: Uuid,
    username: String,
    data: Json<ActivityData>,
    created_at: DateTime<Utc>,
}

impl From<ActivityRow> for Activity {
    fn from(value: ActivityRow) -> Self {
        Self {
            root_id: value.root_id,
            username: value.username,
            data: value.data.0,
            created_at: value.created_at,
        }
    }
}

impl ActivityRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self, activity), fields(root_id = %activity.root_id, user = %activity.username), err(Debug))]
    pub async fn insert(&self, activity: &Activity) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO root_activity (
                root_id,
                username,
                data,
                created_at
            )
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(activity.root_id)
        .bind(&activity.username)
        .bind(Json(&activity.data))
        .bind(activity.created_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(())
    }

    /// Page activities for `root_id`, newest first.
    #[instrument(skip(self), fields(root_id = %root_id), err(Debug))]
    pub async fn list(
        &self,
        root_id: Uuid,
        pagination: Pagination,
    ) -> Result<List<Activity>, AppError> {
        let (limit, offset) = pagination.to_limit_offset();

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT (*)
            FROM root_activity
            WHERE root_id = $1
            "#,
        )
        .bind(root_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        if total == 0 {
            return Ok(List::new(0, Vec::new()));
        }

        let activity = sqlx::query_as!(
            ActivityRow,
            r#"
            SELECT
                root_id,
                username,
                data AS "data: Json<ActivityData>",
                created_at
            FROM root_activity
            WHERE root_id = $1
            ORDER BY created_at DESC, username ASC
            LIMIT $2 OFFSET $3
            "#,
            root_id,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(Activity::from)
        .collect();

        Ok(List::new(total as usize, activity))
    }
}
