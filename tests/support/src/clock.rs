//! Clock double.

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

#[cfg(test)]
mod tests;
