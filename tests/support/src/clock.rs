//! Clock doubles.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use shared_domain::Clock;

/// Always returns the same time.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(SystemTime);

impl FixedClock {
    /// A clock fixed at `time`.
    #[must_use]
    pub const fn at(time: SystemTime) -> Self {
        Self(time)
    }
}

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

/// Returns the time the test set last. A clone is a shared handle on the same time, so a test keeps
/// a clone after it hands the clock to the code under test and moves the time from outside.
#[derive(Debug, Clone)]
pub struct SettableClock(Arc<Mutex<SystemTime>>);

impl SettableClock {
    /// A clock at `time` until [`SettableClock::set`].
    #[must_use]
    pub fn at(time: SystemTime) -> Self {
        Self(Arc::new(Mutex::new(time)))
    }

    /// Moves the clock, and every clone of it, to `time`.
    pub fn set(&self, time: SystemTime) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = time;
    }
}

impl Clock for SettableClock {
    fn now(&self) -> SystemTime {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
