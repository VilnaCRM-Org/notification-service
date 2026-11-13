//! Domain event of the example context.

use std::time::SystemTime;

use shared_domain::{DomainEvent, Uuid};

/// An `ExampleApiResource` was created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleApiResourceCreated {
    /// Identifier of the event.
    pub event_id: Uuid,
    /// Identifier of the created aggregate.
    pub aggregate_id: Uuid,
    /// When the aggregate was created.
    pub occurred_at: SystemTime,
}

impl ExampleApiResourceCreated {
    /// The stable wire name of the event.
    pub const NAME: &'static str = "example_api_resource.created";
}

impl DomainEvent for ExampleApiResourceCreated {
    fn event_id(&self) -> Uuid {
        self.event_id
    }

    fn aggregate_id(&self) -> Uuid {
        self.aggregate_id
    }

    fn occurred_at(&self) -> SystemTime {
        self.occurred_at
    }

    fn event_name(&self) -> &'static str {
        Self::NAME
    }
}
