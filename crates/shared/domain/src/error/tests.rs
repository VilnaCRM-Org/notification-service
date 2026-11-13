use super::error_chain;

#[derive(Debug, thiserror::Error)]
enum Failure {
    #[error("dispatch failure")]
    Dispatch(#[source] Box<Self>),
    #[error("connection refused")]
    Refused,
}

#[test]
fn a_chain_lists_every_error_outermost_first() {
    let cause = Failure::Dispatch(Box::new(Failure::Dispatch(Box::new(Failure::Refused))));

    assert_eq!(
        error_chain(&cause),
        "dispatch failure: dispatch failure: connection refused"
    );
}

#[test]
fn an_error_without_a_source_is_its_own_text() {
    assert_eq!(error_chain(&Failure::Refused), "connection refused");
}
