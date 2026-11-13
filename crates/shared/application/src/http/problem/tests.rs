use http_body_util::BodyExt as _;
use std::fmt;

use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse as _;
use rstest::rstest;
use serde_json::{Value, json};
use shared_domain::BusError;
use test_support::log::LogRecorder;
use tracing::Level;
use validator::Validate;

use super::{
    DependencyFailure, PROBLEM_CONTENT_TYPE, Problem, TYPE_BLANK, TYPE_DEPENDENCY_UNAVAILABLE,
    TYPE_VALIDATION_FAILED, Violation, camel_case,
};

async fn body_of(problem: Problem) -> (StatusCode, String, Value) {
    let response = problem.into_response();
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        content_type,
        serde_json::from_slice(&bytes).unwrap(),
    )
}

fn violation(path: &str, code: &str) -> Violation {
    Violation {
        property_path: path.to_owned(),
        message: format!("{path} is invalid"),
        code: code.to_owned(),
    }
}

#[rstest]
#[case(StatusCode::BAD_REQUEST, "Bad Request")]
#[case(StatusCode::NOT_FOUND, "Not Found")]
#[case(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed")]
#[case(StatusCode::PAYLOAD_TOO_LARGE, "Payload Too Large")]
#[case(StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported Media Type")]
#[case(StatusCode::UNPROCESSABLE_ENTITY, "Unprocessable Entity")]
#[case(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")]
#[case(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable")]
#[tokio::test]
async fn a_new_problem_has_the_canonical_title_and_no_detail(
    #[case] status: StatusCode,
    #[case] title: &str,
) {
    let (actual, content_type, body) = body_of(Problem::new(status)).await;

    assert_eq!(actual, status);
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
    assert_eq!(
        body,
        json!({"type": TYPE_BLANK, "title": title, "status": status.as_u16()})
    );
}

#[test]
fn a_status_without_a_reason_phrase_gets_a_placeholder_title() {
    let status = StatusCode::from_u16(599).unwrap();

    assert_eq!(Problem::new(status).title, "Unknown");
}

#[tokio::test]
async fn the_detail_is_serialized_when_set() {
    let problem = Problem::new(StatusCode::NOT_FOUND).with_detail("gone");

    let (_status, _content_type, body) = body_of(problem).await;

    assert_eq!(
        body,
        json!({"type": "about:blank", "title": "Not Found", "status": 404, "detail": "gone"})
    );
}

#[tokio::test]
async fn an_invalid_status_is_answered_as_an_internal_error() {
    let mut problem = Problem::new(StatusCode::NOT_FOUND);
    problem.status = 0;

    let (status, content_type, _body) = body_of(problem).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
}

#[tokio::test]
async fn validation_failed_sorts_by_path_then_code() {
    let problem = Problem::validation_failed(vec![
        violation("name", "length"),
        violation("itemsPerPage", "range"),
        violation("name", "does_not_contain"),
    ]);

    let (status, content_type, body) = body_of(problem).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
    assert_eq!(
        body,
        json!({
            "type": TYPE_VALIDATION_FAILED,
            "title": "Unprocessable Entity",
            "status": 422,
            "detail": "The request failed validation.",
            "violations": [
                {"propertyPath": "itemsPerPage", "message": "itemsPerPage is invalid", "code": "range"},
                {"propertyPath": "name", "message": "name is invalid", "code": "does_not_contain"},
                {"propertyPath": "name", "message": "name is invalid", "code": "length"},
            ],
        })
    );
}

#[tokio::test]
async fn dependency_unavailable_joins_the_messages() {
    let failure = |dependency: &str, message: &str| DependencyFailure {
        dependency: dependency.to_owned(),
        message: message.to_owned(),
    };
    let problem = Problem::dependency_unavailable(vec![
        failure("database", "Database is not working"),
        failure("cache", "Cache is not working"),
    ]);

    let (status, content_type, body) = body_of(problem).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
    assert_eq!(
        body,
        json!({
            "type": TYPE_DEPENDENCY_UNAVAILABLE,
            "title": "Internal Server Error",
            "status": 500,
            "detail": "Database is not working; Cache is not working",
            "failures": [
                {"dependency": "database", "message": "Database is not working"},
                {"dependency": "cache", "message": "Cache is not working"},
            ],
        })
    );
}

#[test]
fn a_problem_stays_below_128_bytes() {
    assert!(size_of::<Problem>() < 128);
}

#[derive(Debug)]
struct Chained {
    message: &'static str,
    source: Option<Box<Self>>,
}

impl fmt::Display for Chained {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for Chained {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| -> &(dyn std::error::Error + 'static) { source })
    }
}

fn chain() -> Chained {
    Chained {
        message: "query failed",
        source: Some(Box::new(Chained {
            message: "SELECT secret FROM t",
            source: None,
        })),
    }
}

#[tokio::test]
async fn an_internal_problem_hides_the_cause_and_logs_the_chain_once() {
    let (recorder, _guard) = LogRecorder::install();

    let response = Problem::internal(&chain()).into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        bytes,
        r#"{"type":"about:blank","title":"Internal Server Error","status":500}"#
    );
    let lines = recorder.lines();
    assert_eq!(lines.len(), 1);
    let line = lines.first().unwrap();
    assert_eq!(line.level, Level::ERROR);
    assert_eq!(line.message, "request failed");
    assert_eq!(
        line.field("error"),
        Some("query failed: SELECT secret FROM t")
    );
}

#[test]
fn a_rejection_with_a_client_status_keeps_status_and_text() {
    let (recorder, _guard) = LogRecorder::install();

    let problem = Problem::rejection(StatusCode::BAD_REQUEST, "bad input".to_owned(), &chain());

    assert_eq!(
        problem,
        Problem::new(StatusCode::BAD_REQUEST).with_detail("bad input")
    );
    assert_eq!(recorder.lines().len(), 0);
}

#[test]
fn a_rejection_with_a_server_status_is_an_internal_problem() {
    let (recorder, _guard) = LogRecorder::install();

    let problem = Problem::rejection(
        StatusCode::INTERNAL_SERVER_ERROR,
        "driver text".to_owned(),
        &chain(),
    );

    assert_eq!(problem, Problem::new(StatusCode::INTERNAL_SERVER_ERROR));
    assert_eq!(recorder.lines().len(), 1);
}

#[derive(Debug, Validate)]
struct Inner {
    #[validate(length(min = 1, message = "Too short."))]
    display_name: String,
}

#[derive(Debug, Validate)]
struct Outer {
    #[validate(range(min = 1))]
    items_per_page: i32,
    #[validate(nested)]
    inner: Inner,
    #[validate(nested)]
    list: Vec<Inner>,
}

fn invalid_outer() -> Outer {
    let inner = || Inner {
        display_name: String::new(),
    };
    Outer {
        items_per_page: 0,
        inner: inner(),
        list: vec![
            Inner {
                display_name: "ok".to_owned(),
            },
            inner(),
        ],
    }
}

#[test]
fn validator_errors_become_sorted_camel_case_violations() {
    let errors = invalid_outer().validate().unwrap_err();

    let problem = Problem::from(errors);

    assert_eq!(problem.status, 422);
    assert_eq!(problem.r#type, TYPE_VALIDATION_FAILED);
    let paths: Vec<_> = problem
        .violations
        .iter()
        .map(|violation| (violation.property_path.as_str(), violation.code.as_str()))
        .collect();
    assert_eq!(
        paths,
        vec![
            ("inner.displayName", "length"),
            ("itemsPerPage", "range"),
            ("list[1].displayName", "length"),
        ]
    );
}

#[test]
fn a_violation_uses_the_validator_message_or_falls_back_to_the_code() {
    let problem = Problem::from(invalid_outer().validate().unwrap_err());

    let messages: Vec<_> = problem
        .violations
        .iter()
        .map(|violation| violation.message.as_str())
        .collect();

    assert_eq!(messages, vec!["Too short.", "range", "Too short."]);
}

#[rstest]
#[case("name", "name")]
#[case("items_per_page", "itemsPerPage")]
#[case("a_b_c", "aBC")]
#[case("", "")]
fn property_paths_are_camel_case(#[case] snake: &str, #[case] camel: &str) {
    assert_eq!(camel_case(snake), camel);
}

#[derive(Debug)]
struct NotFound;

impl fmt::Display for NotFound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("not found")
    }
}

impl std::error::Error for NotFound {}

impl From<NotFound> for Problem {
    fn from(_error: NotFound) -> Self {
        Self::new(StatusCode::NOT_FOUND).with_detail("it is gone")
    }
}

#[test]
fn a_handler_error_converts_with_its_own_mapping() {
    let (recorder, _guard) = LogRecorder::install();

    let problem = Problem::from(BusError::Handler(NotFound));

    assert_eq!(
        problem,
        Problem::new(StatusCode::NOT_FOUND).with_detail("it is gone")
    );
    assert_eq!(recorder.lines().len(), 0);
}

#[test]
fn a_missing_handler_is_an_internal_problem() {
    let (recorder, _guard) = LogRecorder::install();

    let problem = Problem::from(BusError::<NotFound>::NotRegistered { message: "Ping" });

    assert_eq!(problem, Problem::new(StatusCode::INTERNAL_SERVER_ERROR));
    let lines = recorder.lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines.first().unwrap().field("error"),
        Some("no handler is registered for `Ping`")
    );
}
