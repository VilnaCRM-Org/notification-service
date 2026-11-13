//! Domain event and subscriber contracts.

use crate::error::BoxError;
use crate::value_object::uuid::Uuid;

/// A fact that happened in the domain.
pub trait DomainEvent: std::any::Any + std::fmt::Debug + Send + Sync {
    /// Identifier of the event.
    fn event_id(&self) -> Uuid;
    /// Identifier of the aggregate that recorded it.
    fn aggregate_id(&self) -> Uuid;
    /// When it happened.
    fn occurred_at(&self) -> std::time::SystemTime;
    /// Stable wire name.
    fn event_name(&self) -> &'static str;
}

/// Reacts to one event type.
#[async_trait::async_trait]
pub trait DomainEventSubscriber<E: DomainEvent>: Send + Sync + 'static {
    /// Handles one event.
    async fn handle(&self, event: &E) -> Result<(), BoxError>;
}
