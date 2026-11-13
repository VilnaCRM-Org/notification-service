//! The `ExampleApiResource` aggregate.

use std::time::SystemTime;

use shared_domain::{AggregateRoot, RecordedEvents, Uuid};

use crate::event::ExampleApiResourceCreated;
use crate::value_object::description::ExampleApiResourceDescription;
use crate::value_object::id::ExampleApiResourceId;
use crate::value_object::name::ExampleApiResourceName;

/// The example aggregate: an identifier, a name and a description, built only from valid value
/// objects.
#[derive(Debug)]
pub struct ExampleApiResource {
    id: ExampleApiResourceId,
    name: ExampleApiResourceName,
    description: ExampleApiResourceDescription,
    events: RecordedEvents,
}

impl ExampleApiResource {
    /// Builds a new aggregate and records `ExampleApiResourceCreated`.
    #[must_use]
    pub fn create(
        id: ExampleApiResourceId,
        name: ExampleApiResourceName,
        description: ExampleApiResourceDescription,
        event_id: Uuid,
        occurred_at: SystemTime,
    ) -> Self {
        let mut resource = Self::restore(id, name, description);
        resource.events.record(ExampleApiResourceCreated {
            event_id,
            aggregate_id: id.value(),
            occurred_at,
        });
        resource
    }

    /// Rebuilds a stored aggregate; records nothing.
    #[must_use]
    pub fn restore(
        id: ExampleApiResourceId,
        name: ExampleApiResourceName,
        description: ExampleApiResourceDescription,
    ) -> Self {
        Self {
            id,
            name,
            description,
            events: RecordedEvents::default(),
        }
    }

    /// The identifier.
    #[must_use]
    pub const fn id(&self) -> ExampleApiResourceId {
        self.id
    }

    /// The name.
    #[must_use]
    pub const fn name(&self) -> &ExampleApiResourceName {
        &self.name
    }

    /// The description.
    #[must_use]
    pub const fn description(&self) -> &ExampleApiResourceDescription {
        &self.description
    }
}

impl AggregateRoot for ExampleApiResource {
    fn recorded_events(&mut self) -> &mut RecordedEvents {
        &mut self.events
    }
}

#[cfg(test)]
mod tests;
