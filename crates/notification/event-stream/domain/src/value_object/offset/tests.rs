use rstest::rstest;

use super::Offset;

#[rstest]
#[case(0)]
#[case(1043)]
#[case(u64::MAX)]
fn keeps_its_value(#[case] value: u64) {
    assert_eq!(Offset::new(value).value(), value);
}

#[rstest]
#[case(0, 1)]
#[case(1, 2)]
#[case(1042, 1043)]
#[case(u64::MAX - 1, u64::MAX)]
#[case(u64::MAX, u64::MAX)]
fn the_sequence_is_the_offset_plus_one_saturating(#[case] offset: u64, #[case] sequence: u64) {
    assert_eq!(Offset::new(offset).sequence().value(), sequence);
}

#[test]
fn orders_by_the_number() {
    assert!(Offset::new(1) > Offset::new(0));
    assert!(Offset::new(9) < Offset::new(10));
}
