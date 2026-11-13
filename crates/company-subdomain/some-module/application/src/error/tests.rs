use std::error::Error as _;

use rstest::rstest;
use shared_application::http::{Problem, TYPE_BLANK, TYPE_VALIDATION_FAILED, Violation};
use shared_domain::{InvalidPageRequest, Uuid};
use some_module_domain::{ExampleApiResourceId, InvalidExampleApiResource, RepositoryError};
use test_support::log::LogRecorder;

use super::ExampleApiResourceError;

fn id() -> ExampleApiResourceId {
    ExampleApiResourceId::new(Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90").unwrap())
}

fn violation(property_path: &str, code: &str, message: &str) -> Violation {
    Violation {
        property_path: property_path.to_owned(),
        message: message.to_owned(),
        code: code.to_owned(),
    }
}

#[rstest]
#[case::name_length(InvalidExampleApiResource::NameLength, "name", "length")]
#[case::name_nul(InvalidExampleApiResource::NameContainsNul, "name", "does_not_contain")]
#[case::description_length(InvalidExampleApiResource::DescriptionLength, "description", "length")]
#[case::description_nul(
    InvalidExampleApiResource::DescriptionContainsNul,
    "description",
    "does_not_contain"
)]
fn an_invalid_value_is_a_validation_problem_with_one_violation(
    #[case] invalid: InvalidExampleApiResource,
    #[case] property_path: &str,
    #[case] code: &str,
) {
    let problem = Problem::from(ExampleApiResourceError::Invalid(invalid));

    assert_eq!(problem.status, 422);
    assert_eq!(problem.r#type, TYPE_VALIDATION_FAILED);
    assert_eq!(
        problem.violations,
        [violation(property_path, code, &invalid.to_string())]
    );
}

#[rstest]
#[case::page(InvalidPageRequest::Page, "page")]
#[case::items_per_page(InvalidPageRequest::ItemsPerPage, "itemsPerPage")]
fn an_invalid_page_is_a_validation_problem_with_a_range_violation(
    #[case] invalid: InvalidPageRequest,
    #[case] property_path: &str,
) {
    let problem = Problem::from(ExampleApiResourceError::InvalidPage(invalid));

    assert_eq!(problem.status, 422);
    assert_eq!(problem.r#type, TYPE_VALIDATION_FAILED);
    assert_eq!(
        problem.violations,
        [violation(property_path, "range", &invalid.to_string())]
    );
}

#[rstest]
#[case::already_exists(ExampleApiResourceError::AlreadyExists(id()))]
#[case::storage(ExampleApiResourceError::Storage("disk on fire".into()))]
fn a_server_side_error_is_the_internal_problem_and_is_logged(
    #[case] error: ExampleApiResourceError,
) {
    let (recorder, _guard) = LogRecorder::install();

    let problem = Problem::from(error);

    assert_eq!(problem.status, 500);
    assert_eq!(problem.r#type, TYPE_BLANK);
    assert_eq!(problem.detail, None);
    assert_eq!(problem.violations, Vec::<Violation>::new());
    assert_eq!(recorder.lines().len(), 1);
}

#[test]
fn messages_and_sources_are_those_of_the_listing() {
    let storage = ExampleApiResourceError::storage(RepositoryError::Unavailable("x".into()));

    assert_eq!(
        ExampleApiResourceError::AlreadyExists(id()).to_string(),
        "ExampleApiResource 0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90 already exists."
    );
    assert_eq!(
        storage.to_string(),
        "the example API resource storage is unavailable"
    );
    assert_eq!(
        storage.source().unwrap().to_string(),
        "the repository is unavailable"
    );
    assert_eq!(
        ExampleApiResourceError::from(InvalidExampleApiResource::NameLength).to_string(),
        InvalidExampleApiResource::NameLength.to_string()
    );
    assert_eq!(
        ExampleApiResourceError::from(InvalidPageRequest::Page).to_string(),
        InvalidPageRequest::Page.to_string()
    );
}
