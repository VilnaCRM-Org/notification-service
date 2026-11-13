//! Cache double.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use shared_domain::{Cache, CacheError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Behaviour {
    Working,
    Failing,
    Forgetful,
    Corrupting,
}

/// A cache with a fixed behaviour.
#[derive(Debug)]
pub struct StubCache {
    behaviour: Behaviour,
    entries: Mutex<HashMap<String, Vec<u8>>>,
}

impl StubCache {
    fn with(behaviour: Behaviour) -> Self {
        Self {
            behaviour,
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn entries(&self) -> MutexGuard<'_, HashMap<String, Vec<u8>>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Stores in a map.
    #[must_use]
    pub fn working() -> Self {
        Self::with(Behaviour::Working)
    }

    /// Errors on every call.
    #[must_use]
    pub fn failing() -> Self {
        Self::with(Behaviour::Failing)
    }

    /// Accepts writes and returns nothing.
    #[must_use]
    pub fn forgetful() -> Self {
        Self::with(Behaviour::Forgetful)
    }

    /// Stores in a map and returns a different value than the one stored.
    #[must_use]
    pub fn corrupting() -> Self {
        Self::with(Behaviour::Corrupting)
    }
}

#[async_trait]
impl Cache for StubCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        match self.behaviour {
            Behaviour::Failing => Err(CacheError("stub cache failure".into())),
            Behaviour::Forgetful => Ok(None),
            Behaviour::Working => Ok(self.entries().get(key).cloned()),
            Behaviour::Corrupting => Ok(self.entries().get(key).map(|value| {
                let mut corrupted = value.clone();
                corrupted.push(0xFF);
                corrupted
            })),
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>, _ttl: Duration) -> Result<(), CacheError> {
        match self.behaviour {
            Behaviour::Failing => Err(CacheError("stub cache failure".into())),
            Behaviour::Forgetful => Ok(()),
            Behaviour::Working | Behaviour::Corrupting => {
                self.entries().insert(key.to_owned(), value);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests;
