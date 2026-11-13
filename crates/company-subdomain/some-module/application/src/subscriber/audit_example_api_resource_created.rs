//! Writes an audit log entry for every created resource.

use shared_domain::{BoxError, DomainEventSubscriber};
use some_module_domain::ExampleApiResourceCreated;

/// Logs `ExampleApiResourceCreated` events on the `audit` target.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuditExampleApiResourceCreated;

#[async_trait::async_trait]
impl DomainEventSubscriber<ExampleApiResourceCreated> for AuditExampleApiResourceCreated {
    async fn handle(&self, event: &ExampleApiResourceCreated) -> Result<(), BoxError> {
        log_event(event);
        Ok(())
    }
}

fn log_event(event: &ExampleApiResourceCreated) {
    tracing::info!(target: "audit", event_name = ExampleApiResourceCreated::NAME, event_id = %event.event_id, aggregate_id = %event.aggregate_id, occurred_at = %humantime::format_rfc3339_micros(event.occurred_at), "domain event");
}

#[cfg(test)]
mod tests;
