use std::time::{Duration, SystemTime};

use shared_domain::{AggregateRoot as _, DomainEvent as _, Uuid};

use super::ExampleApiResource;
use crate::event::ExampleApiResourceCreated;
use crate::value_object::description::ExampleApiResourceDescription;
use crate::value_object::id::ExampleApiResourceId;
use crate::value_object::name::ExampleApiResourceName;

fn uuid(text: &str) -> Uuid {
    Uuid::parse(text).unwrap()
}

fn time() -> SystemTime {
    SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(1_700_000_000))
        .unwrap()
}

fn id() -> ExampleApiResourceId {
    ExampleApiResourceId::new(uuid("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90"))
}

fn name() -> ExampleApiResourceName {
    ExampleApiResourceName::new("Name".to_owned()).unwrap()
}

fn description() -> ExampleApiResourceDescription {
    ExampleApiResourceDescription::new("Description".to_owned()).unwrap()
}

fn event_id() -> Uuid {
    uuid("0192f3a0-7b1c-7d2e-8f3a-000000000001")
}

#[test]
fn create_records_exactly_one_created_event() {
    let mut resource = ExampleApiResource::create(id(), name(), description(), event_id(), time());

    let events = resource.pull_domain_events();

    assert_eq!(events.len(), 1);
    let event = events.first().unwrap();
    assert_eq!(event.event_id(), event_id());
    assert_eq!(event.aggregate_id(), id().value());
    assert_eq!(event.occurred_at(), time());
    assert_eq!(event.event_name(), "example_api_resource.created");
    assert!(resource.pull_domain_events().is_empty());
}

#[test]
fn the_created_event_carries_its_fields() {
    let event = ExampleApiResourceCreated {
        event_id: event_id(),
        aggregate_id: id().value(),
        occurred_at: time(),
    };

    assert_eq!(event.event_id(), event_id());
    assert_eq!(event.aggregate_id(), id().value());
    assert_eq!(event.occurred_at(), time());
    assert_eq!(event.event_name(), ExampleApiResourceCreated::NAME);
    assert_eq!(
        ExampleApiResourceCreated::NAME,
        "example_api_resource.created"
    );
}

#[test]
fn restore_records_no_event() {
    let mut resource = ExampleApiResource::restore(id(), name(), description());

    assert!(resource.pull_domain_events().is_empty());
}

#[test]
fn exposes_what_it_was_built_from() {
    let resource = ExampleApiResource::create(id(), name(), description(), event_id(), time());

    assert_eq!(resource.id(), id());
    assert_eq!(resource.name(), &name());
    assert_eq!(resource.description(), &description());
}
