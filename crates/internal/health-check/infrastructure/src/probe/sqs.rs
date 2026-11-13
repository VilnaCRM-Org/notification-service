//! SQS probe.

use aws_sdk_sqs::Client;
use health_check_domain::{Dependency, HealthProbe};
use shared_domain::BoxError;

/// Checks that the broker answers a read-only `ListQueues` call; nothing is created, changed or deleted.
#[derive(Debug, Clone)]
pub struct SqsProbe {
    client: Client,
}

impl SqsProbe {
    /// Creates a probe over the SQS client of the service.
    #[must_use]
    pub const fn new(client: Client) -> Self {
        Self { client }
    }

    /// Probe logic, kept out of the `#[async_trait]` body so coverage and complexity see it.
    async fn probe(&self) -> Result<(), BoxError> {
        self.client.list_queues().max_results(1).send().await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl HealthProbe for SqsProbe {
    fn dependency(&self) -> Dependency {
        Dependency::MessageBroker
    }

    async fn check(&self) -> Result<(), BoxError> {
        self.probe().await
    }
}

#[cfg(test)]
mod tests;
