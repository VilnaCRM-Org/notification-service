use rstest::rstest;

use super::{InvalidCursor, InvalidEvent};

#[rstest]
#[case(InvalidEvent::TypeLength, "length")]
#[case(InvalidEvent::TypePattern, "pattern")]
#[case(InvalidEvent::SequenceZero, "range")]
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
#[case(InvalidEvent::SequenceZero, "A sequence must be 1 or greater.")]
fn states_the_rule(#[case] error: InvalidEvent, #[case] message: &str) {
    assert_eq!(error.to_string(), message);
}

#[test]
fn an_invalid_cursor_states_the_rule() {
    assert_eq!(
        InvalidCursor.to_string(),
        "A cursor must be a decimal unsigned 64-bit integer."
    );
}
