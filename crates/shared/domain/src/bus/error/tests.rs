use super::{BusError, ErasedBusError};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("boom")]
struct Boom;

#[derive(Debug, thiserror::Error)]
#[error("other")]
struct Other;

#[test]
fn a_missing_handler_names_the_message() {
    let error: BusError<Boom> = BusError::NotRegistered { message: "Ping" };

    assert_eq!(error.to_string(), "no handler is registered for `Ping`");
}

#[test]
fn a_handler_error_keeps_its_own_message() {
    let error = BusError::Handler(Boom);

    assert_eq!(error.to_string(), "boom");
}

#[test]
fn restores_a_missing_handler() {
    let typed = ErasedBusError::NotRegistered { message: "Ping" }.into_typed::<Boom>("Other");

    assert!(matches!(typed, BusError::NotRegistered { message: "Ping" }));
}

#[test]
fn restores_the_handler_error_type() {
    let typed = ErasedBusError::Handler(Box::new(Boom)).into_typed::<Boom>("Ping");

    assert!(matches!(typed, BusError::Handler(Boom)));
}

#[test]
fn reports_a_missing_handler_when_the_error_has_another_type() {
    let typed = ErasedBusError::Handler(Box::new(Other)).into_typed::<Boom>("Ping");

    assert!(matches!(typed, BusError::NotRegistered { message: "Ping" }));
}
