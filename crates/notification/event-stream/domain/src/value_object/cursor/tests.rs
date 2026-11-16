use proptest::prelude::any;
use rstest::rstest;

use super::Cursor;
use crate::error::InvalidCursor;
use crate::value_object::sequence::Sequence;

#[rstest]
#[case("0", 0)]
#[case("1043", 1043)]
#[case("007", 7)]
#[case("18446744073709551615", u64::MAX)]
fn parses_decimal_digits(#[case] text: &str, #[case] value: u64) {
    assert_eq!(Cursor::parse(text), Ok(Cursor::new(value)));
}

#[rstest]
#[case::negative("-1")]
#[case::plus_sign("+1")]
#[case::leading_blank(" 1")]
#[case::trailing_blank("1 ")]
#[case::empty("")]
#[case::letter("1a")]
#[case::hexadecimal("0x10")]
#[case::overflow("18446744073709551616")]
#[case::fraction("1.0")]
#[case::non_ascii_digit("\u{661}")]
fn rejects_anything_but_digits_of_a_u64(#[case] text: &str) {
    assert_eq!(Cursor::parse(text), Err(InvalidCursor));
}

#[test]
fn start_is_zero() {
    assert_eq!(Cursor::START.value(), 0);
}

#[rstest]
#[case(1)]
#[case(1043)]
#[case(u64::MAX)]
fn the_value_is_the_number_it_was_made_from(#[case] value: u64) {
    assert_eq!(Cursor::new(value).value(), value);
}

#[test]
fn a_parsed_cursor_has_the_value_of_its_digits() {
    assert_eq!(Cursor::parse("1043").map(Cursor::value), Ok(1043));
}

#[rstest]
#[case(0)]
#[case(1043)]
#[case(u64::MAX)]
fn the_next_offset_is_the_same_number(#[case] value: u64) {
    assert_eq!(Cursor::new(value).next_offset().value(), value);
}

#[rstest]
#[case(10, 3, 7)]
#[case(10, 10, 0)]
#[case(10, 11, 0)]
#[case(0, 1, 0)]
#[case(5, 0, 5)]
#[case(u64::MAX, u64::MAX, 0)]
fn rewinding_saturates_at_zero(#[case] value: u64, #[case] n: u64, #[case] expected: u64) {
    assert_eq!(Cursor::new(value).rewound_by(n), Cursor::new(expected));
}

#[rstest]
#[case(10, 3, 13)]
#[case(0, 0, 0)]
#[case(u64::MAX - 1, 1, u64::MAX)]
#[case(u64::MAX - 1, 2, u64::MAX)]
#[case(u64::MAX, u64::MAX, u64::MAX)]
fn advancing_saturates_at_the_maximum(#[case] value: u64, #[case] n: u64, #[case] expected: u64) {
    assert_eq!(Cursor::new(value).advanced_by(n), Cursor::new(expected));
}

#[rstest]
#[case(10, 3, 7)]
#[case(10, 9, 1)]
#[case(10, 10, 0)]
#[case(10, 11, 0)]
#[case(0, u64::MAX, 0)]
#[case(u64::MAX, 0, u64::MAX)]
fn the_distance_is_zero_when_lower_is_not_below(
    #[case] value: u64,
    #[case] lower: u64,
    #[case] expected: u64,
) {
    assert_eq!(
        Cursor::new(value).distance_from(Cursor::new(lower)),
        expected
    );
}

#[rstest]
#[case(1)]
#[case(1043)]
#[case(u64::MAX)]
fn a_sequence_converts_to_the_same_number(#[case] value: u64) {
    assert_eq!(
        Cursor::from(Sequence::new(value).unwrap()),
        Cursor::new(value)
    );
}

#[test]
fn displays_the_decimal_number() {
    assert_eq!(Cursor::new(1043).to_string(), "1043");
    assert_eq!(Cursor::START.to_string(), "0");
}

#[test]
fn orders_by_the_number() {
    assert!(Cursor::new(2) > Cursor::new(1));
}

proptest::proptest! {
    #[test]
    fn parsing_the_display_gives_the_cursor_back(value in any::<u64>()) {
        let cursor = Cursor::new(value);
        proptest::prop_assert_eq!(Cursor::parse(&cursor.to_string()), Ok(cursor));
    }

    #[test]
    fn agrees_with_an_oracle_on_any_text(text in "[0-9+ a-x-]{0,22}") {
        let expected = if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) {
            text.parse::<u64>().ok().map(Cursor::new).ok_or(InvalidCursor)
        } else {
            Err(InvalidCursor)
        };
        proptest::prop_assert_eq!(Cursor::parse(&text), expected);
    }
}
