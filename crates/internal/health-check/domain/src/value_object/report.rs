//! The outcome of probing every dependency.

use std::time::Duration;

use shared_domain::BoxError;

use super::dependency::Dependency;

/// Why a probe counts as failed.
#[derive(Debug, thiserror::Error)]
pub enum ProbeFailureCause {
    /// The probe returned an error.
    #[error("probe failed")]
    Failed(#[source] BoxError),
    /// The probe did not finish within its deadline.
    #[error("probe timed out after {0:?}")]
    TimedOut(Duration),
}

/// One dependency that failed its probe.
#[derive(Debug)]
pub struct ProbeFailure {
    /// The failing dependency.
    pub dependency: Dependency,
    /// Why it failed; logged, never returned to the client.
    pub cause: ProbeFailureCause,
}

/// Every failure of one health check, in registration order.
#[derive(Debug, Default)]
pub struct HealthReport {
    failures: Vec<ProbeFailure>,
}

impl HealthReport {
    /// A report that holds `failures`.
    #[must_use]
    pub const fn new(failures: Vec<ProbeFailure>) -> Self {
        Self { failures }
    }

    /// Whether no probe failed.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.failures.is_empty()
    }

    /// The failures in registration order.
    #[must_use]
    pub fn failures(&self) -> &[ProbeFailure] {
        &self.failures
    }
}

#[cfg(test)]
mod tests;
