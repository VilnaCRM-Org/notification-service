use std::time::SystemTime;

use shared_domain::{BusError, Command, CommandBus, DomainEvent, EventBus, Query, QueryBus, Uuid};

use super::{FakeCommandBus, FakeQueryBus, PublishedEvent, RecordingEventBus};
use crate::block_on;

#[derive(Debug, PartialEq, Eq)]
struct Rejected;

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("rejected")
    }
}

impl std::error::Error for Rejected {}

struct Rename(String);

impl Command for Rename {
    type Error = Rejected;
    const NAME: &'static str = "Rename";
}

struct Other;

impl Command for Other {
    type Error = Rejected;
    const NAME: &'static str = "Other";
}

struct Lookup(u32);

impl Query for Lookup {
    type Output = String;
    type Error = Rejected;
    const NAME: &'static str = "Lookup";
}

struct OtherQuery;

impl Query for OtherQuery {
    type Output = String;
    type Error = Rejected;
    const NAME: &'static str = "OtherQuery";
}

#[derive(Debug)]
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
        SystemTime::UNIX_EPOCH
    }

    fn event_name(&self) -> &'static str {
        "aggregate.happened"
    }
}

fn event(id: u128) -> Box<dyn DomainEvent> {
    Box::new(Happened { id, aggregate: 99 })
}

#[test]
fn the_command_bus_answers_with_the_closure_result() {
    let bus = FakeCommandBus::handling(|command: Rename| {
        if command.0.is_empty() {
            Err(Rejected)
        } else {
            Ok(())
        }
    });
    let bus: &dyn CommandBus = &bus;

    assert!(block_on(bus.dispatch(Rename("a".into()))).is_ok());
    assert!(matches!(
        block_on(bus.dispatch(Rename(String::new()))),
        Err(BusError::Handler(Rejected))
    ));
}

#[test]
fn the_command_bus_records_every_dispatched_command() {
    let fake = FakeCommandBus::handling(|_command: Rename| Ok(()));

    let bus: &dyn CommandBus = &fake;
    block_on(bus.dispatch(Rename("a".into()))).unwrap();
    block_on(bus.dispatch(Rename("b".into()))).unwrap();

    assert_eq!(fake.dispatched(), ["Rename", "Rename"]);
}

#[test]
fn the_command_bus_does_not_know_a_command_of_another_type() {
    let fake = FakeCommandBus::handling(|_command: Rename| Ok(()));

    let bus: &dyn CommandBus = &fake;
    let result = block_on(bus.dispatch(Other));

    assert!(matches!(
        result,
        Err(BusError::NotRegistered { message: "Other" })
    ));
    assert_eq!(fake.dispatched(), ["Other"]);
}

#[test]
fn the_unregistered_command_bus_answers_not_registered_and_records() {
    let fake = FakeCommandBus::unregistered();

    let bus: &dyn CommandBus = &fake;
    let result = block_on(bus.dispatch(Rename("a".into())));

    assert!(matches!(
        result,
        Err(BusError::NotRegistered { message: "Rename" })
    ));
    assert_eq!(fake.dispatched(), ["Rename"]);
}

#[test]
fn the_query_bus_answers_with_the_closure_result() {
    let fake = FakeQueryBus::answering(|query: Lookup| {
        if query.0 == 0 {
            Err(Rejected)
        } else {
            Ok(format!("item {}", query.0))
        }
    });

    let bus: &dyn QueryBus = &fake;

    assert_eq!(block_on(bus.ask(Lookup(3))).unwrap(), "item 3");
    assert!(matches!(
        block_on(bus.ask(Lookup(0))),
        Err(BusError::Handler(Rejected))
    ));
    assert_eq!(fake.asked(), ["Lookup", "Lookup"]);
}

#[test]
fn the_query_bus_does_not_know_a_query_of_another_type() {
    let fake = FakeQueryBus::answering(|query: Lookup| Ok(query.0.to_string()));

    let bus: &dyn QueryBus = &fake;
    let result = block_on(bus.ask(OtherQuery));

    assert!(matches!(
        result,
        Err(BusError::NotRegistered {
            message: "OtherQuery"
        })
    ));
}

#[test]
fn the_unregistered_query_bus_answers_not_registered() {
    let fake = FakeQueryBus::unregistered();

    let bus: &dyn QueryBus = &fake;
    let result = block_on(bus.ask(Lookup(1)));

    assert!(matches!(
        result,
        Err(BusError::NotRegistered { message: "Lookup" })
    ));
    assert_eq!(fake.asked(), ["Lookup"]);
}

#[test]
fn the_buses_are_debug() {
    assert!(format!("{:?}", FakeCommandBus::unregistered()).contains("FakeCommandBus"));
    assert!(format!("{:?}", FakeQueryBus::unregistered()).contains("FakeQueryBus"));
}

#[test]
fn the_event_bus_records_published_events_in_order() {
    let fake = RecordingEventBus::new();

    let bus: &dyn EventBus = &fake;
    block_on(bus.publish(vec![event(1), event(2)])).unwrap();
    block_on(bus.publish(Vec::new())).unwrap();
    block_on(bus.publish(vec![event(3)])).unwrap();

    let ids: Vec<u128> = fake
        .published()
        .iter()
        .map(|published| published.event_id.as_uuid().as_u128())
        .collect();
    assert_eq!(ids, [1, 2, 3]);
    assert_eq!(
        fake.published().first(),
        Some(&PublishedEvent {
            event_name: "aggregate.happened",
            event_id: Uuid::from_uuid(uuid::Uuid::from_u128(1)),
            aggregate_id: Uuid::from_uuid(uuid::Uuid::from_u128(99)),
        })
    );
}

#[test]
fn the_failing_event_bus_records_and_returns_one_failure_per_event() {
    let fake = RecordingEventBus::failing();

    let bus: &dyn EventBus = &fake;
    let error = block_on(bus.publish(vec![event(1), event(2)])).unwrap_err();

    assert_eq!(error.failures.len(), 2);
    assert_eq!(error.failures[0].event_name, "aggregate.happened");
    assert_eq!(error.failures[1].event_id.as_uuid().as_u128(), 2);
    assert_eq!(
        error.failures[0].source.to_string(),
        "recording bus failure"
    );
    assert_eq!(fake.published().len(), 2);
}

#[test]
fn the_failing_event_bus_succeeds_when_nothing_is_published() {
    let fake = RecordingEventBus::failing();

    let bus: &dyn EventBus = &fake;

    assert!(block_on(bus.publish(Vec::new())).is_ok());
    assert_eq!(fake.published().len(), 0);
}
