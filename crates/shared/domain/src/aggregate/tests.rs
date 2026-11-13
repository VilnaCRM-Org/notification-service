use std::time::{Duration, SystemTime};

use super::{AggregateRoot, RecordedEvents};
use crate::event::DomainEvent;
use crate::value_object::uuid::Uuid;

fn expected_time() -> SystemTime {
    SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(60))
        .unwrap()
}

#[derive(Debug, PartialEq, Eq)]
struct Happened {
    id: u128,
    aggregate: u128,
}

impl DomainEvent for Happened {
    fn event_id(&self) -> Uuid {
        Uuid::from_uuid(uuid::Uuid::from_u128(self.id))
    }

    fn aggregate_id(&self) -> Uuid {
        Uuid::from_uuid(uuid::Uuid::from_u128(self.aggregate))
    }

    fn occurred_at(&self) -> SystemTime {
        expected_time()
    }

    fn event_name(&self) -> &'static str {
        "test_aggregate.happened"
    }
}

#[derive(Debug, Default)]
struct TestAggregate {
    events: RecordedEvents,
}

impl TestAggregate {
    fn happen(&mut self, id: u128) {
        self.events.record(Happened { id, aggregate: 99 });
    }
}

impl AggregateRoot for TestAggregate {
    fn recorded_events(&mut self) -> &mut RecordedEvents {
        &mut self.events
    }
}

fn ids(events: &[Box<dyn DomainEvent>]) -> Vec<Uuid> {
    events.iter().map(|event| event.event_id()).collect()
}

#[test]
fn returns_the_recorded_events_in_recording_order_and_then_none() {
    let mut aggregate = TestAggregate::default();
    aggregate.happen(1);
    aggregate.happen(2);
    aggregate.happen(3);

    let first = aggregate.pull_domain_events();
    let second = aggregate.pull_domain_events();

    let expected: Vec<Uuid> = (1..=3)
        .map(|id| Uuid::from_uuid(uuid::Uuid::from_u128(id)))
        .collect();
    assert_eq!(ids(&first), expected);
    assert!(second.is_empty());
}

#[test]
fn a_new_aggregate_has_no_events() {
    let mut aggregate = TestAggregate::default();

    assert!(aggregate.pull_domain_events().is_empty());
}

#[test]
fn records_again_after_a_pull() {
    let mut aggregate = TestAggregate::default();
    aggregate.happen(1);
    drop(aggregate.pull_domain_events());
    aggregate.happen(2);

    let events = aggregate.pull_domain_events();

    assert_eq!(
        ids(&events),
        vec![Uuid::from_uuid(uuid::Uuid::from_u128(2))]
    );
}

#[test]
fn pull_drains_the_store_directly() {
    let mut store = RecordedEvents::default();
    store.record(Happened {
        id: 5,
        aggregate: 6,
    });

    assert_eq!(store.pull().len(), 1);
    assert!(store.pull().is_empty());
}

#[test]
fn an_event_exposes_its_identity_time_and_name() {
    let mut aggregate = TestAggregate::default();
    aggregate.happen(7);

    let events = aggregate.pull_domain_events();
    let event = events.first().unwrap();

    assert_eq!(event.event_id(), Uuid::from_uuid(uuid::Uuid::from_u128(7)));
    assert_eq!(
        event.aggregate_id(),
        Uuid::from_uuid(uuid::Uuid::from_u128(99))
    );
    assert_eq!(event.occurred_at(), expected_time());
    assert_eq!(event.event_name(), "test_aggregate.happened");
}
