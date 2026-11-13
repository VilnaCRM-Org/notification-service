//! In-process cache with a time to live per entry.

use std::time::{Duration, Instant};

use shared_domain::{Cache, CacheError};

#[derive(Clone)]
struct Entry {
    value: Vec<u8>,
    ttl: Duration,
}

/// Gives every entry the time to live it was stored with.
struct EntryExpiry;

impl moka::Expiry<String, Entry> for EntryExpiry {
    fn expire_after_create(
        &self,
        _key: &String,
        entry: &Entry,
        _created_at: Instant,
    ) -> Option<Duration> {
        Some(entry.ttl)
    }

    fn expire_after_update(
        &self,
        _key: &String,
        entry: &Entry,
        _updated_at: Instant,
        _duration_until_expiry: Option<Duration>,
    ) -> Option<Duration> {
        Some(entry.ttl)
    }
}

/// Bounded in-process cache backed by `moka`.
pub struct MokaCache {
    entries: moka::future::Cache<String, Entry>,
}

impl MokaCache {
    /// Maximum number of entries held at once.
    pub const MAX_ENTRIES: u64 = 10_000;

    /// An empty cache.
    #[must_use]
    pub fn new() -> Self {
        let entries = moka::future::Cache::builder()
            .max_capacity(Self::MAX_ENTRIES)
            .expire_after(EntryExpiry)
            .build();
        Self { entries }
    }

    async fn read(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        Ok(self.entries.get(key).await.map(|entry| entry.value))
    }

    async fn write(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<(), CacheError> {
        self.entries
            .insert(key.to_owned(), Entry { value, ttl })
            .await;
        Ok(())
    }
}

impl Default for MokaCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MokaCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MokaCache")
            .field("entries", &self.entries.entry_count())
            .finish()
    }
}

#[async_trait::async_trait]
impl Cache for MokaCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        self.read(key).await
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<(), CacheError> {
        self.write(key, value, ttl).await
    }
}

#[cfg(test)]
mod tests;
