//! In-process event bus.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use shared_domain::{
    BoxError, DomainEvent, DomainEventSubscriber, EventBus, EventBusError, SubscriberFailure,
};

/// A subscriber received an event of a type it did not subscribe to.
#[derive(Debug, thiserror::Error)]
#[error("a subscriber of `{expected}` received another event type")]
struct EventTypeMismatch {
    expected: &'static str,
}

#[async_trait::async_trait]
trait ErasedSubscriber: Send + Sync {
    async fn handle(&self, event: &dyn DomainEvent) -> Result<(), BoxError>;
}

struct SubscriberAdapter<E, S> {
    subscriber: S,
    event: PhantomData<fn() -> E>,
}

impl<E: DomainEvent, S: DomainEventSubscriber<E>> SubscriberAdapter<E, S> {
    async fn run(&self, event: &dyn DomainEvent) -> Result<(), BoxError> {
        let any: &dyn Any = event;
        let event = any.downcast_ref::<E>().ok_or_else(|| EventTypeMismatch {
            expected: std::any::type_name::<E>(),
        })?;
        self.subscriber.handle(event).await
    }
}

#[async_trait::async_trait]
impl<E: DomainEvent, S: DomainEventSubscriber<E>> ErasedSubscriber for SubscriberAdapter<E, S> {
    async fn handle(&self, event: &dyn DomainEvent) -> Result<(), BoxError> {
        self.run(event).await
    }
}

/// Event bus that runs every subscriber of every published event, in registration order.
#[derive(Default)]
pub struct InMemoryEventBus {
    subscribers: HashMap<TypeId, Vec<Arc<dyn ErasedSubscriber>>>,
}

impl InMemoryEventBus {
    /// An empty bus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a subscriber of event type `E`.
    #[must_use]
    pub fn subscribe<E: DomainEvent, S: DomainEventSubscriber<E>>(mut self, subscriber: S) -> Self {
        self.subscribers
            .entry(TypeId::of::<E>())
            .or_default()
            .push(Arc::new(SubscriberAdapter {
                subscriber,
                event: PhantomData,
            }));
        self
    }

    async fn deliver(&self, events: Vec<Box<dyn DomainEvent>>) -> Result<(), EventBusError> {
        let mut failures = Vec::new();
        for event in &events {
            self.deliver_one(event.as_ref(), &mut failures).await;
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(EventBusError { failures })
        }
    }

    async fn deliver_one(&self, event: &dyn DomainEvent, failures: &mut Vec<SubscriberFailure>) {
        let any: &dyn Any = event;
        let Some(subscribers) = self.subscribers.get(&any.type_id()) else {
            return;
        };
        for subscriber in subscribers {
            if let Err(source) = subscriber.handle(event).await {
                failures.push(SubscriberFailure {
                    event_name: event.event_name(),
                    event_id: event.event_id(),
                    source,
                });
            }
        }
    }
}

impl std::fmt::Debug for InMemoryEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryEventBus")
            .field("event_types", &self.subscribers.len())
            .finish()
    }
}

#[async_trait::async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, events: Vec<Box<dyn DomainEvent>>) -> Result<(), EventBusError> {
        self.deliver(events).await
    }
}

#[cfg(test)]
mod tests;
