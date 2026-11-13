//! Aggregate root contract.

use crate::event::DomainEvent;

/// Events recorded by an aggregate and not yet published.
#[derive(Debug, Default)]
pub struct RecordedEvents(Vec<Box<dyn DomainEvent>>);

impl RecordedEvents {
    /// Appends an event.
    pub fn record<E: DomainEvent>(&mut self, event: E) {
        self.0.push(Box::new(event));
    }

    /// Drains the events in recording order.
    #[must_use]
    pub fn pull(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.0)
    }
}

/// An aggregate that records domain events.
pub trait AggregateRoot {
    /// The event store of the aggregate.
    fn recorded_events(&mut self) -> &mut RecordedEvents;

    /// Drains the recorded events.
    #[must_use]
    fn pull_domain_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        self.recorded_events().pull()
    }
}

#[cfg(test)]
mod tests;
