use crate::error::AppError;
use deadpool_redis::redis::AsyncCommands;

/// Valkey cache. Valkey speaks the Redis protocol, so the client is the Redis one.
#[derive(Clone)]
pub struct ValkeyCache {
    pool: deadpool_redis::Pool,
}

impl ValkeyCache {
    pub fn connect(url: &str) -> Result<Self, AppError> {
        let pool = deadpool_redis::Config::from_url(url)
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .map_err(|error| {
                tracing::error!(%error, "unable to create the valkey pool");
                AppError::Internal
            })?;
        Ok(Self { pool })
    }

    async fn connection(&self) -> Result<deadpool_redis::Connection, AppError> {
        self.pool.get().await.map_err(|error| {
            tracing::error!(%error, "valkey pool error");
            AppError::Internal
        })
    }

    pub async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), AppError> {
        let mut conn = self.connection().await?;
        conn.set_ex::<_, _, ()>(key, value, ttl_secs)
            .await
            .map_err(|error| {
                tracing::error!(%error, "valkey write failed");
                AppError::Internal
            })
    }

    pub async fn get_del(&self, key: &str) -> Result<Option<String>, AppError> {
        let mut conn = self.connection().await?;
        conn.get_del(key).await.map_err(|error| {
            tracing::error!(%error, "valkey read failed");
            AppError::Internal
        })
    }

    #[allow(dead_code)]
    pub async fn del(&self, key: &str) -> Result<(), AppError> {
        let mut conn = self.connection().await?;
        conn.del::<_, ()>(key).await.map_err(|error| {
            tracing::error!(%error, "valkey delete failed");
            AppError::Internal
        })
    }
}
