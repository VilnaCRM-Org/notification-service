//! The health use case: every probe runs concurrently under a deadline.

use std::sync::Arc;
use std::time::Duration;

use futures_util::future::join_all;
use health_check_domain::{Dependency, HealthProbe, HealthReport, ProbeFailure, ProbeFailureCause};
use shared_domain::error_chain;

/// Probes every registered dependency and reports the ones that failed.
#[derive(Debug)]
pub struct HealthService {
    probes: Vec<Arc<dyn HealthProbe>>,
    probe_timeout: Duration,
}

impl HealthService {
    /// A service that runs `probes` and gives each one `probe_timeout` to answer.
    #[must_use]
    pub const fn new(probes: Vec<Arc<dyn HealthProbe>>, probe_timeout: Duration) -> Self {
        Self {
            probes,
            probe_timeout,
        }
    }

    /// Runs all probes concurrently; the report lists the failures in registration order.
    pub async fn check(&self) -> HealthReport {
        let outcomes = join_all(self.probes.iter().map(|probe| self.run(probe.as_ref()))).await;
        HealthReport::new(outcomes.into_iter().flatten().collect())
    }

    /// Runs one probe under the deadline; a failure or a timeout is logged and returned.
    async fn run(&self, probe: &dyn HealthProbe) -> Option<ProbeFailure> {
        let cause = match tokio::time::timeout(self.probe_timeout, probe.check()).await {
            Ok(Ok(())) => return None,
            Ok(Err(source)) => ProbeFailureCause::Failed(source),
            Err(_elapsed) => ProbeFailureCause::TimedOut(self.probe_timeout),
        };
        let failure = ProbeFailure {
            dependency: probe.dependency(),
            cause,
        };
        log_failure(failure.dependency, &error_chain(&failure.cause));
        Some(failure)
    }
}

fn log_failure(dependency: Dependency, cause: &str) {
    tracing::warn!(dependency = dependency.identifier(), cause = %cause, "health probe failed");
}

#[cfg(test)]
mod tests;
