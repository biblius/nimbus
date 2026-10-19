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

/// Process-local cache; always available, and the fallback when Valkey fails.
#[derive(Clone, Default)]
pub struct MemoryCache {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
}

impl MemoryCache {
    pub fn new() -> Self {
        Self::default()
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
}
