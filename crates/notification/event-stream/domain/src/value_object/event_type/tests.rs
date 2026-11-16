use std::cmp::Ordering;

use proptest::prelude::{Just, Strategy, any, prop_oneof};
use rstest::rstest;

use super::EventType;
use crate::error::InvalidEvent;

const MAX: usize = 128;

/// An oracle written apart from the matcher: the first character is a lower-case letter and the
/// name, cut at every separator, gives only non-empty runs of lower-case letters and digits.
fn grammar(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && value.split(['.', '_', '-']).all(|run| {
            !run.is_empty()
                && run
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn expected(value: &str) -> Result<String, InvalidEvent> {
    if !(1..=MAX).contains(&value.chars().count()) {
        Err(InvalidEvent::TypeLength)
    } else if grammar(value) {
        Ok(value.to_owned())
    } else {
        Err(InvalidEvent::TypePattern)
    }
}

fn accepted(value: &str) -> Result<String, InvalidEvent> {
    EventType::new(value.to_owned()).map(|name| name.as_str().to_owned())
}

#[test]
fn the_limit_is_128() {
    assert_eq!(EventType::MAX_CHARS, MAX);
}

#[rstest]
#[case(1)]
#[case(MAX)]
fn accepts_the_boundary_lengths(#[case] length: usize) {
    let value = "a".repeat(length);

    assert_eq!(accepted(&value), Ok(value));
}

#[rstest]
#[case(0)]
#[case(MAX + 1)]
fn rejects_a_length_outside_1_to_128(#[case] length: usize) {
    assert_eq!(accepted(&"a".repeat(length)), Err(InvalidEvent::TypeLength));
}

#[test]
fn counts_characters_not_bytes() {
    let at_limit = "\u{e9}".repeat(MAX);

    assert_eq!(accepted(&at_limit), Err(InvalidEvent::TypePattern));
    assert_eq!(
        accepted(&"\u{e9}".repeat(MAX + 1)),
        Err(InvalidEvent::TypeLength)
    );
}

#[test]
fn reports_the_length_before_the_pattern() {
    assert_eq!(
        accepted(&"A".repeat(MAX + 1)),
        Err(InvalidEvent::TypeLength)
    );
}

#[rstest]
#[case::upper_case("User.created")]
#[case::leading_digit("1a")]
#[case::double_separator("a..b")]
#[case::trailing_separator("a.")]
#[case::leading_separator(".a")]
#[case::blank("a b")]
#[case::slash("a/b")]
#[case::separator_only("-")]
#[case::mixed_separators("a._b")]
#[case::trailing_upper_case("aB")]
#[case::non_ascii("a\u{e9}")]
fn rejects_a_name_outside_the_grammar(#[case] value: &str) {
    assert_eq!(accepted(value), Err(InvalidEvent::TypePattern));
}

#[rstest]
#[case("a")]
#[case("a1")]
#[case("user.created")]
#[case("user_created-v2.x")]
#[case("z9")]
#[case("a-0")]
fn accepts_a_name_of_the_grammar(#[case] value: &str) {
    assert_eq!(accepted(value), Ok(value.to_owned()));
}

#[test]
fn displays_the_name() {
    let name = EventType::new("order.paid".to_owned()).unwrap();

    assert_eq!(name.to_string(), "order.paid");
}

#[rstest]
#[case("order-paid", "order.paid", Ordering::Less)]
#[case("order.paid", "order_paid", Ordering::Less)]
#[case("order_paid", "user.created", Ordering::Less)]
#[case("a", "a1", Ordering::Less)]
#[case("b", "a1", Ordering::Greater)]
#[case("a.b", "a.b", Ordering::Equal)]
fn orders_by_the_bytes_of_the_names(
    #[case] left: &str,
    #[case] right: &str,
    #[case] order: Ordering,
) {
    let left_name = EventType::new(left.to_owned()).unwrap();
    let right_name = EventType::new(right.to_owned()).unwrap();

    assert_eq!(left_name.cmp(&right_name), order);
    assert_eq!(
        left_name.cmp(&right_name),
        left.as_bytes().cmp(right.as_bytes())
    );
}

const GRAMMAR: &str = "[a-z][a-z0-9]{0,8}([._-][a-z0-9]{1,8}){0,6}";

/// A valid name with one character replaced, inserted or removed.
fn mutated() -> impl Strategy<Value = String> {
    let character = prop_oneof![
        Just('.'),
        Just('_'),
        Just('-'),
        Just('A'),
        Just('0'),
        Just('a'),
        Just(' '),
        Just('/'),
        any::<char>(),
    ];
    (GRAMMAR, any::<usize>(), character, 0_u8..3).prop_map(
        |(name, position, character, operation)| {
            let mut characters: Vec<char> = name.chars().collect();
            let at = position
                .checked_rem(characters.len().saturating_add(1))
                .unwrap_or(0);
            match operation {
                0 if at < characters.len() => characters[at] = character,
                1 if at < characters.len() => {
                    characters.remove(at);
                }
                _ => characters.insert(at, character),
            }
            characters.into_iter().collect()
        },
    )
}

proptest::proptest! {
    #[test]
    fn accepts_every_name_generated_from_the_grammar(value in GRAMMAR) {
        let answer = accepted(&value);
        proptest::prop_assert_eq!(answer, Ok(value));
    }

    #[test]
    fn agrees_with_the_grammar_on_mutated_names(value in mutated()) {
        proptest::prop_assert_eq!(accepted(&value), expected(&value));
    }

    #[test]
    fn agrees_with_the_grammar_on_any_text(value in ".{0,140}") {
        proptest::prop_assert_eq!(accepted(&value), expected(&value));
    }

    #[test]
    fn agrees_with_the_grammar_on_names_near_the_limit(value in "[a-z][a-z0-9._-]{120,130}") {
        proptest::prop_assert_eq!(accepted(&value), expected(&value));
    }
}
