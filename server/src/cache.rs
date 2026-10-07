//! Exactly one backend is compiled in, so `Cache` is a concrete type rather than a trait object.

#[cfg(any(feature = "memory-cache", test))]
mod memory;

#[cfg(feature = "valkey")]
mod valkey;

#[cfg(any(feature = "memory-cache", test))]
pub use memory::MemoryCache;

#[cfg(feature = "valkey")]
pub use valkey::ValkeyCache;

#[cfg(feature = "memory-cache")]
pub type Cache = MemoryCache;

#[cfg(feature = "valkey")]
pub type Cache = ValkeyCache;
