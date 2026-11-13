//! Event bus port.

use crate::error::BoxError;
use crate::event::DomainEvent;
use crate::value_object::uuid::Uuid;

/// The failure of one subscriber for one event.
#[derive(Debug, thiserror::Error)]
#[error("a subscriber of `{event_name}` failed for event {event_id}")]
pub struct SubscriberFailure {
    /// Wire name of the event.
    pub event_name: &'static str,
    /// Identifier of the event.
    pub event_id: Uuid,
    /// What the subscriber returned.
    #[source]
    pub source: BoxError,
}

/// Every subscriber failure of one `publish` call, in invocation order; never empty.
#[derive(Debug, thiserror::Error)]
#[error("{} event subscriber(s) failed", .failures.len())]
pub struct EventBusError {
    /// The failures, in invocation order.
    pub failures: Vec<SubscriberFailure>,
}

/// Publishes domain events to their subscribers.
#[async_trait::async_trait]
pub trait EventBus: std::fmt::Debug + Send + Sync {
    /// Publishes the events in order.
    ///
    /// # Errors
    ///
    /// Returns every subscriber failure of the call.
    async fn publish(&self, events: Vec<Box<dyn DomainEvent>>) -> Result<(), EventBusError>;
}

#[cfg(test)]
mod tests;
