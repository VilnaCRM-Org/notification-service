//! System clock adapter.

use std::time::SystemTime;

use shared_domain::Clock;

/// The wall clock of the host.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    #[expect(
        clippy::disallowed_methods,
        reason = "the adapter that owns the wall clock"
    )]
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}
