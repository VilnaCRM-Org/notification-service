//! Cache port.

use crate::error::BoxError;

/// A cache operation failed.
#[derive(Debug, thiserror::Error)]
#[error("cache operation failed")]
pub struct CacheError(#[source] pub BoxError);

/// Key-value cache with a mandatory time to live.
#[async_trait::async_trait]
pub trait Cache: std::fmt::Debug + Send + Sync {
    /// Reads a value.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;
    /// Stores a value for `ttl`.
    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: std::time::Duration,
    ) -> Result<(), CacheError>;
}
