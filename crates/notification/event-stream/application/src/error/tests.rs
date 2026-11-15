use rstest::rstest;
use shared_application::http::{Problem, TYPE_VALIDATION_FAILED, Violation};
use shared_domain::InvalidPageRequest;

use super::EventStreamError;

#[rstest]
#[case::page(InvalidPageRequest::Page, "page", "page must be 1 or greater")]
#[case::items_per_page(
    InvalidPageRequest::ItemsPerPage,
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
fn an_invalid_page_is_a_validation_problem_with_a_range_violation(
    #[case] invalid: InvalidPageRequest,
    #[case] property_path: &str,
    #[case] message: &str,
) {
    let problem = Problem::from(EventStreamError::InvalidPage(invalid));

    assert_eq!(problem.status, 422);
    assert_eq!(problem.r#type, TYPE_VALIDATION_FAILED);
    assert_eq!(
        problem.violations,
        [Violation {
            property_path: property_path.to_owned(),
            message: message.to_owned(),
            code: "range".to_owned(),
        }]
    );
}

#[test]
fn the_error_reads_like_the_page_request_error() {
    let error = EventStreamError::from(InvalidPageRequest::Page);

    assert_eq!(error.to_string(), InvalidPageRequest::Page.to_string());
}
