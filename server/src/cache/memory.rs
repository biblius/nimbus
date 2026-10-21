use crate::error::AppError;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

struct Entry {
    value: String,
    expires_at: Instant,
}

/// Process-local cache used when Valkey is not configured.
#[derive(Clone, Default)]
pub struct MemoryCache {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
}

const SWEEP_INTERVAL: Duration = Duration::from_secs(10 * 60);

impl MemoryCache {
    pub fn new() -> Self {
        let cache = Self::default();
        cache.spawn_sweeper();
        cache
    }

    pub async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), AppError> {
        let mut entries = self.entries.lock().await;

        entries.insert(
            key.to_string(),
            Entry {
                value: value.to_string(),
                expires_at: Instant::now() + Duration::from_secs(ttl_secs),
            },
        );

        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        let mut entries = self.entries.lock().await;

        let Some(entry) = entries.get(key) else {
            return Ok(None);
        };

        if entry.expires_at <= Instant::now() {
            entries.remove(key);
            return Ok(None);
        }

        Ok(Some(entry.value.clone()))
    }

    pub async fn get_del(&self, key: &str) -> Result<Option<String>, AppError> {
        let mut entries = self.entries.lock().await;

        let Some(entry) = entries.remove(key) else {
            return Ok(None);
        };

        if entry.expires_at <= Instant::now() {
            return Ok(None);
        }

        Ok(Some(entry.value))
    }

    pub async fn del(&self, key: &str) -> Result<(), AppError> {
        let mut entries = self.entries.lock().await;

        entries.remove(key);

        Ok(())
    }

    fn spawn_sweeper(&self) {
        let cache = self.clone();

        tokio::spawn(async move {
            let mut tick = tokio::time::interval(SWEEP_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            tick.tick().await;

            loop {
                tick.tick().await;
                cache.sweep_expired().await;
            }
        });
    }

    async fn sweep_expired(&self) {
        let mut entries = self.entries.lock().await;
        let now = Instant::now();
        let before = entries.len();

        entries.retain(|_, entry| entry.expires_at > now);

        let removed = before - entries.len();
        if removed > 0 {
            tracing::debug!(removed, "swept expired memory cache entries");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryCache;

    #[tokio::test]
    async fn sweep_drops_expired_entries() {
        let cache = MemoryCache::default();

        cache.set_ex("gone", "v", 0).await.unwrap();
        cache.set_ex("stay", "v", 60).await.unwrap();
        cache.sweep_expired().await;

        assert!(cache.get("gone").await.unwrap().is_none());
        assert_eq!(cache.get("stay").await.unwrap().as_deref(), Some("v"));
    }
}
