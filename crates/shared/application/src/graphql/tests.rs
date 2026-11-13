use async_graphql::Value;
use axum::http::StatusCode;
use rstest::rstest;
use serde_json::json;

use super::problem_to_error;
use crate::http::{Problem, Violation};

fn extension(error: &async_graphql::Error, name: &str) -> Option<Value> {
    error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get(name))
        .cloned()
}

fn violation(path: &str, code: &str) -> Violation {
    Violation {
        property_path: path.to_owned(),
        message: format!("{path} is invalid"),
        code: code.to_owned(),
    }
}

#[test]
fn a_validation_problem_keeps_the_detail_and_the_violations() {
    let problem =
        Problem::validation_failed(vec![violation("page", "range"), violation("id", "uuid")]);

    let error = problem_to_error(problem);

    assert_eq!(error.message, "The request failed validation.");
    assert_eq!(
        extension(&error, "code"),
        Some(Value::String("VALIDATION_FAILED".to_owned()))
    );
    assert_eq!(
        extension(&error, "violations"),
        Some(
            Value::try_from(json!([
                {"propertyPath": "id", "message": "id is invalid", "code": "uuid"},
                {"propertyPath": "page", "message": "page is invalid", "code": "range"},
            ]))
            .unwrap()
        )
    );
}

#[test]
fn a_422_problem_without_detail_falls_back_to_its_title() {
    let error = problem_to_error(Problem::new(StatusCode::UNPROCESSABLE_ENTITY));

    assert_eq!(error.message, "Unprocessable Entity");
    assert_eq!(
        extension(&error, "code"),
        Some(Value::String("VALIDATION_FAILED".to_owned()))
    );
    assert_eq!(
        extension(&error, "violations"),
        Some(Value::List(Vec::new()))
    );
}

#[rstest]
#[case(StatusCode::NOT_FOUND)]
#[case(StatusCode::INTERNAL_SERVER_ERROR)]
#[case(StatusCode::SERVICE_UNAVAILABLE)]
fn every_other_status_is_an_internal_server_error(#[case] status: StatusCode) {
    let error = problem_to_error(Problem::new(status).with_detail("secret detail"));

    assert_eq!(error.message, "Internal server error");
    assert_eq!(
        extension(&error, "code"),
        Some(Value::String("INTERNAL_SERVER_ERROR".to_owned()))
    );
    assert_eq!(extension(&error, "violations"), None);
}
