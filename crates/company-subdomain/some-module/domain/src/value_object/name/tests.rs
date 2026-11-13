use rstest::rstest;

use super::ExampleApiResourceName;
use crate::error::InvalidExampleApiResource;

const MAX: usize = 255;

#[test]
fn the_limit_is_255() {
    assert_eq!(ExampleApiResourceName::MAX_CHARS, MAX);
}

#[rstest]
#[case(0)]
#[case(MAX + 1)]
fn rejects_a_length_outside_1_to_255(#[case] length: usize) {
    assert_eq!(
        ExampleApiResourceName::new("a".repeat(length)),
        Err(InvalidExampleApiResource::NameLength)
    );
}

#[rstest]
#[case(1)]
#[case(MAX)]
fn accepts_the_boundary_lengths(#[case] length: usize) {
    let value = "a".repeat(length);

    let accepted = ExampleApiResourceName::new(value.clone()).unwrap();

    assert_eq!(accepted.as_str(), value);
}

#[test]
fn counts_unicode_scalar_values_not_bytes() {
    let at_limit = "\u{1F600}".repeat(MAX);
    let over_limit = "\u{1F600}".repeat(MAX + 1);

    assert_eq!(
        ExampleApiResourceName::new(at_limit.clone())
            .unwrap()
            .as_str(),
        at_limit
    );
    assert_eq!(
        ExampleApiResourceName::new(over_limit),
        Err(InvalidExampleApiResource::NameLength)
    );
}

#[test]
fn rejects_the_nul_character_anywhere() {
    for value in ["\0", "\0abc", "ab\0c", "abc\0"] {
        assert_eq!(
            ExampleApiResourceName::new(value.to_owned()),
            Err(InvalidExampleApiResource::NameContainsNul)
        );
    }
}

#[test]
fn accepts_a_nul_free_value_at_the_limit_after_a_nul_free_prefix() {
    let value = format!("{}\u{1}", "a".repeat(MAX - 1));

    assert!(ExampleApiResourceName::new(value).is_ok());
}

#[test]
fn reports_the_length_before_the_nul_character() {
    let value = format!("\0{}", "a".repeat(MAX));

    assert_eq!(
        ExampleApiResourceName::new(value),
        Err(InvalidExampleApiResource::NameLength)
    );
}

#[rstest]
#[case("   ")]
#[case("\t\n")]
#[case("  padded  ")]
fn keeps_whitespace_untouched(#[case] value: &str) {
    assert_eq!(
        ExampleApiResourceName::new(value.to_owned())
            .unwrap()
            .as_str(),
        value
    );
}

proptest::proptest! {
    #[test]
    fn accepts_exactly_the_values_that_meet_the_invariants(value in ".{0,275}") {
        let characters = value.chars().count();
        let expected = if !(1..=MAX).contains(&characters) {
            Err(InvalidExampleApiResource::NameLength)
        } else if value.contains('\0') {
            Err(InvalidExampleApiResource::NameContainsNul)
        } else {
            Ok(value.clone())
        };

        let actual = ExampleApiResourceName::new(value).map(|accepted| accepted.as_str().to_owned());

        proptest::prop_assert_eq!(actual, expected);
    }
}
