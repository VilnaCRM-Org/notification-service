use rstest::rstest;

use super::Sequence;
use crate::error::InvalidEvent;

#[test]
fn rejects_zero() {
    assert_eq!(Sequence::new(0), Err(InvalidEvent::SequenceZero));
    assert_eq!(Sequence::new(0).unwrap_err().code(), "range");
}

#[rstest]
#[case(1)]
#[case(2)]
#[case(1043)]
#[case(u64::MAX)]
fn accepts_one_and_above(#[case] value: u64) {
    assert_eq!(Sequence::new(value).map(Sequence::value), Ok(value));
}

#[rstest]
#[case(0, 1)]
#[case(41, 42)]
#[case(u64::MAX - 1, u64::MAX)]
#[case(u64::MAX, u64::MAX)]
fn the_following_sequence_is_one_more_and_saturates(#[case] value: u64, #[case] next: u64) {
    assert_eq!(Sequence::following(value).value(), next);
}

#[test]
fn displays_the_decimal_number() {
    assert_eq!(Sequence::new(1043).unwrap().to_string(), "1043");
    assert_eq!(
        Sequence::new(u64::MAX).unwrap().to_string(),
        "18446744073709551615"
    );
}

#[test]
fn orders_by_the_number() {
    assert!(Sequence::new(2).unwrap() > Sequence::new(1).unwrap());
    assert!(Sequence::new(9).unwrap() < Sequence::new(10).unwrap());
}
