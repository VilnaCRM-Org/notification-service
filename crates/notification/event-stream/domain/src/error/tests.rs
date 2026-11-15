use rstest::rstest;

use super::InvalidEvent;

#[rstest]
#[case(InvalidEvent::TypeLength, "length")]
#[case(InvalidEvent::TypePattern, "pattern")]
fn names_the_violation_code(#[case] error: InvalidEvent, #[case] code: &str) {
    assert_eq!(error.code(), code);
}

#[rstest]
#[case(
    InvalidEvent::TypeLength,
    "An event type must contain between 1 and 128 characters."
)]
#[case(
    InvalidEvent::TypePattern,
    "An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$."
)]
fn states_the_rule(#[case] error: InvalidEvent, #[case] message: &str) {
    assert_eq!(error.to_string(), message);
}
