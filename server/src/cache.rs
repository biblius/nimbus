//! Global process cache.
//!
//! Always includes an in-memory store. When a Valkey URL is configured, Valkey
//! is tried first and memory is used when Valkey fails.

mod memory;
mod valkey;

use crate::error::AppError;

pub use memory::MemoryCache;
pub use valkey::ValkeyCache;

pub type Cache = NimbusCache;

/// Prefer Valkey when configured; fall back to the in-memory store on failure.
#[derive(Clone)]
pub struct NimbusCache {
    memory: MemoryCache,
    valkey: Option<ValkeyCache>,
}

impl NimbusCache {
    pub fn new() -> Self {
        Self {
            memory: MemoryCache::new(),
            valkey: None,
        }
    }

    pub fn connect(url: &str) -> Result<Self, AppError> {
        Ok(Self {
            memory: MemoryCache::new(),
            valkey: Some(ValkeyCache::connect(url)?),
        })
    }

    pub async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), AppError> {
        if let Some(valkey) = &self.valkey {
            match valkey.set_ex(key, value, ttl_secs).await {
                Ok(()) => {
                    tracing::debug!(key, "cache set_ex via valkey");
                    return Ok(());
                }
                Err(_) => {
                    tracing::debug!(key, "valkey set_ex failed; falling back to memory");
                }
            }
        }

        tracing::debug!(key, "cache set_ex via memory");
        self.memory.set_ex(key, value, ttl_secs).await
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        if let Some(valkey) = &self.valkey {
            match valkey.get(key).await {
                Ok(value) => {
                    tracing::debug!(key, "cache get via valkey");
                    return Ok(value);
                }
                Err(_) => {
                    tracing::debug!(key, "valkey get failed; falling back to memory");
                }
            }
        }

        tracing::debug!(key, "cache get via memory");
        self.memory.get(key).await
    }

    pub async fn get_del(&self, key: &str) -> Result<Option<String>, AppError> {
        if let Some(valkey) = &self.valkey {
            match valkey.get_del(key).await {
                Ok(value) => {
                    tracing::debug!(key, "cache get_del via valkey");
                    return Ok(value);
                }
                Err(_) => {
                    tracing::debug!(key, "valkey get_del failed; falling back to memory");
                }
            }
        }

        tracing::debug!(key, "cache get_del via memory");
        self.memory.get_del(key).await
    }

    pub async fn del(&self, key: &str) -> Result<(), AppError> {
        if let Some(valkey) = &self.valkey {
            match valkey.del(key).await {
                Ok(()) => {
                    tracing::debug!(key, "cache del via valkey");
                    return Ok(());
                }
                Err(_) => {
                    tracing::debug!(key, "valkey del failed; falling back to memory");
                }
            }
        }

        tracing::debug!(key, "cache del via memory");
        self.memory.del(key).await
    }
}

impl Default for NimbusCache {
    fn default() -> Self {
        Self::new()
    }
}
