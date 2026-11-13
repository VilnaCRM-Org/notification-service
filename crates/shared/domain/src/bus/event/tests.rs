use std::error::Error as _;

use super::{EventBusError, SubscriberFailure};
use crate::error::BoxError;
use crate::value_object::uuid::Uuid;

#[derive(Debug, thiserror::Error)]
#[error("mail server down")]
struct MailDown;

fn event_id(bits: u128) -> Uuid {
    Uuid::from_uuid(uuid::Uuid::from_u128(bits))
}

fn failure(event_name: &'static str, bits: u128) -> SubscriberFailure {
    let source: BoxError = Box::new(MailDown);
    SubscriberFailure {
        event_name,
        event_id: event_id(bits),
        source,
    }
}

#[test]
fn a_subscriber_failure_names_the_event_and_its_id() {
    let failure = failure("order.placed", 0x1234);

    assert_eq!(
        failure.to_string(),
        "a subscriber of `order.placed` failed for event 00000000-0000-0000-0000-000000001234"
    );
}

#[test]
fn a_subscriber_failure_keeps_the_subscriber_error_as_its_source() {
    let failure = failure("order.placed", 1);

    assert_eq!(failure.source().unwrap().to_string(), "mail server down");
}

#[test]
fn an_event_bus_error_counts_its_failures_in_the_message() {
    let one = EventBusError {
        failures: vec![failure("order.placed", 1)],
    };
    let two = EventBusError {
        failures: vec![failure("order.placed", 1), failure("order.paid", 2)],
    };

    assert_eq!(one.to_string(), "1 event subscriber(s) failed");
    assert_eq!(two.to_string(), "2 event subscriber(s) failed");
}

#[test]
fn an_event_bus_error_carries_every_failure_in_order() {
    let error = EventBusError {
        failures: vec![
            failure("order.placed", 1),
            failure("order.paid", 2),
            failure("order.shipped", 3),
        ],
    };

    let carried: Vec<(&str, Uuid)> = error
        .failures
        .iter()
        .map(|failure| (failure.event_name, failure.event_id))
        .collect();
    assert_eq!(
        carried,
        vec![
            ("order.placed", event_id(1)),
            ("order.paid", event_id(2)),
            ("order.shipped", event_id(3)),
        ]
    );
}
