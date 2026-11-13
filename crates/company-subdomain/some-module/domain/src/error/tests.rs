use std::error::Error as _;

use rstest::rstest;

use super::InvalidExampleApiResource;
use crate::repository::RepositoryError;

#[rstest]
#[case(InvalidExampleApiResource::NameLength, "name", "length")]
#[case(InvalidExampleApiResource::NameContainsNul, "name", "does_not_contain")]
#[case(InvalidExampleApiResource::DescriptionLength, "description", "length")]
#[case(
    InvalidExampleApiResource::DescriptionContainsNul,
    "description",
    "does_not_contain"
)]
fn exposes_the_property_path_and_the_code(
    #[case] error: InvalidExampleApiResource,
    #[case] property_path: &str,
    #[case] code: &str,
) {
    assert_eq!(error.property_path(), property_path);
    assert_eq!(error.code(), code);
}

#[rstest]
#[case(
    InvalidExampleApiResource::NameLength,
    "This value must contain between 1 and 255 characters."
)]
#[case(
    InvalidExampleApiResource::NameContainsNul,
    "This value must not contain the NUL character."
)]
#[case(
    InvalidExampleApiResource::DescriptionLength,
    "This value must contain between 1 and 1024 characters."
)]
#[case(
    InvalidExampleApiResource::DescriptionContainsNul,
    "This value must not contain the NUL character."
)]
fn displays_the_violation_message(#[case] error: InvalidExampleApiResource, #[case] text: &str) {
    assert_eq!(error.to_string(), text);
}

#[test]
fn displays_and_chains_its_errors() {
    let unavailable = RepositoryError::Unavailable("disk on fire".into());

    assert_eq!(
        RepositoryError::AlreadyExists.to_string(),
        "the resource already exists"
    );
    assert_eq!(unavailable.to_string(), "the repository is unavailable");
    assert_eq!(unavailable.source().unwrap().to_string(), "disk on fire");
    assert!(RepositoryError::AlreadyExists.source().is_none());
}
