//! Global process cache.
//!
//! Uses the in-memory store when no Valkey URL is configured. When Valkey is
//! enabled, all operations go to Valkey and errors are returned to the caller
//! (no silent fallthrough to memory).

mod memory;
mod valkey;

use crate::error::AppError;

pub use memory::MemoryCache;
pub use valkey::ValkeyCache;

pub type Cache = NimbusCache;

/// Prefer Valkey when configured.
///
/// When Valkey is configured, the cache can fail. We never fall through
/// to memory when Valkey is enabled, `VALKEY_URL` must be unset to use memory.
///
/// Since Nimbus cache operations are very simple Valkey is not necessary.
///
/// Even so, Valkey is always the preferred way to cache.
#[derive(Clone)]
pub struct NimbusCache {
    /// Default cache implementation, used for minimal setup.
    memory: MemoryCache,

    /// When present, the memory cache is essentially ignored.
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
                Err(e) => {
                    tracing::debug!(error=%e, key, "valkey set_ex failed");
                    return Err(e);
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
                Err(e) => {
                    tracing::debug!(error=%e, key, "valkey get failed");
                    return Err(e);
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
                Err(e) => {
                    tracing::debug!(error=%e, key, "valkey get_del failed");
                    return Err(e);
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
                Err(e) => {
                    tracing::debug!(error=%e, key, "valkey del failed");
                    return Err(e);
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

#[cfg(test)]
mod tests {
    use super::NimbusCache;
    use crate::error::AppError;

    #[tokio::test]
    async fn nimbus_valkey_errors_are_not_swallowed() {
        // Port 1 is almost never a Redis listener; the pool creates, the command fails.
        let cache = NimbusCache::connect("redis://127.0.0.1:1").expect("pool create");

        let set = cache.set_ex("k", "v", 60).await;
        assert!(matches!(set, Err(AppError::Internal)));

        let get = cache.get("k").await;
        assert!(matches!(get, Err(AppError::Internal)));

        let get_del = cache.get_del("k").await;
        assert!(matches!(get_del, Err(AppError::Internal)));

        let del = cache.del("k").await;
        assert!(matches!(del, Err(AppError::Internal)));
    }

    #[tokio::test]
    async fn nimbus_memory_mode_roundtrips() {
        let cache = NimbusCache::new();
        cache.set_ex("k", "v", 60).await.unwrap();
        assert_eq!(cache.get("k").await.unwrap().as_deref(), Some("v"));
        assert_eq!(cache.get_del("k").await.unwrap().as_deref(), Some("v"));
        assert_eq!(cache.get("k").await.unwrap(), None);
    }

    /// Valkey container tests ride the `postgres` feature so `sqlite` stays Docker-free.
    #[cfg(feature = "postgres")]
    mod valkey_parity {
        use super::super::{MemoryCache, ValkeyCache};
        use std::time::Duration;
        use testcontainers::{
            ContainerAsync, GenericImage,
            core::{IntoContainerPort, WaitFor},
            runners::AsyncRunner,
        };

        async fn start_valkey() -> (ValkeyCache, ContainerAsync<GenericImage>) {
            let container = GenericImage::new("valkey/valkey", "8")
                .with_exposed_port(6379.tcp())
                .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
                .start()
                .await
                .expect("valkey container error");

            let host = container.get_host().await.unwrap();
            let port = container.get_host_port_ipv4(6379).await.unwrap();
            let url = format!("redis://{host}:{port}");
            let cache = ValkeyCache::connect(&url).expect("test valkey connection");

            (cache, container)
        }

        /// Run the same operation sequence against both backends and compare each result.
        async fn assert_identical(memory: &MemoryCache, valkey: &ValkeyCache, prefix: &str) {
            let key = |name: &str| format!("{prefix}:{name}");

            assert_eq!(
                memory.get(&key("missing")).await.unwrap(),
                valkey.get(&key("missing")).await.unwrap(),
            );
            assert_eq!(
                memory.get_del(&key("missing")).await.unwrap(),
                valkey.get_del(&key("missing")).await.unwrap(),
            );

            memory.set_ex(&key("a"), "one", 60).await.unwrap();
            valkey.set_ex(&key("a"), "one", 60).await.unwrap();

            assert_eq!(
                memory.get(&key("a")).await.unwrap(),
                valkey.get(&key("a")).await.unwrap(),
            );
            assert_eq!(
                memory.get(&key("a")).await.unwrap().as_deref(),
                Some("one"),
            );

            memory.set_ex(&key("a"), "two", 60).await.unwrap();
            valkey.set_ex(&key("a"), "two", 60).await.unwrap();

            assert_eq!(
                memory.get(&key("a")).await.unwrap(),
                valkey.get(&key("a")).await.unwrap(),
            );

            memory.del(&key("a")).await.unwrap();
            valkey.del(&key("a")).await.unwrap();

            assert_eq!(memory.get(&key("a")).await.unwrap(), None);
            assert_eq!(
                memory.get(&key("a")).await.unwrap(),
                valkey.get(&key("a")).await.unwrap(),
            );

            memory.set_ex(&key("b"), "pop", 60).await.unwrap();
            valkey.set_ex(&key("b"), "pop", 60).await.unwrap();

            assert_eq!(
                memory.get_del(&key("b")).await.unwrap(),
                valkey.get_del(&key("b")).await.unwrap(),
            );
            assert_eq!(
                memory.get_del(&key("b")).await.unwrap(),
                valkey.get_del(&key("b")).await.unwrap(),
            );

            assert_eq!(memory.get(&key("b")).await.unwrap(), None);

            memory.set_ex(&key("empty"), "", 60).await.unwrap();
            valkey.set_ex(&key("empty"), "", 60).await.unwrap();

            assert_eq!(
                memory.get(&key("empty")).await.unwrap(),
                valkey.get(&key("empty")).await.unwrap(),
            );

            memory.set_ex(&key("utf8"), "café ☁️", 60).await.unwrap();
            valkey.set_ex(&key("utf8"), "café ☁️", 60).await.unwrap();

            assert_eq!(
                memory.get(&key("utf8")).await.unwrap(),
                valkey.get(&key("utf8")).await.unwrap(),
            );

            memory.set_ex(&key("ttl"), "soon", 1).await.unwrap();
            valkey.set_ex(&key("ttl"), "soon", 1).await.unwrap();

            assert_eq!(
                memory.get(&key("ttl")).await.unwrap().as_deref(),
                Some("soon"),
            );
            assert_eq!(
                valkey.get(&key("ttl")).await.unwrap().as_deref(),
                Some("soon"),
            );

            tokio::time::sleep(Duration::from_millis(1100)).await;

            assert_eq!(
                memory.get(&key("ttl")).await.unwrap(),
                valkey.get(&key("ttl")).await.unwrap(),
            );
            assert_eq!(memory.get(&key("ttl")).await.unwrap(), None);
        }

        #[tokio::test]
        async fn memory_and_valkey_behave_identically() {
            let memory = MemoryCache::default();
            let (valkey, _container) = start_valkey().await;

            assert_identical(&memory, &valkey, "parity").await;
        }
    }
}
