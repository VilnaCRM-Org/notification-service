use axum::Router;
use axum::body::Body;
use axum::http::header::{ALLOW, CONTENT_TYPE};
use axum::http::{Method, Request, StatusCode};
use axum::response::IntoResponse as _;
use axum::routing::get;
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use test_support::log::LogRecorder;
use tower::ServiceExt as _;
use tracing::Level;

use super::{method_not_allowed, not_found, panic_response};

async fn body_of(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn not_found_is_the_404_problem() {
    let response = not_found().await.into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        body_of(response).await,
        json!({
            "type": "about:blank",
            "title": "Not Found",
            "status": 404,
            "detail": "No route matches the request path.",
        })
    );
}

#[tokio::test]
async fn method_not_allowed_is_the_405_problem() {
    let response = method_not_allowed().await.into_response();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        body_of(response).await,
        json!({"type": "about:blank", "title": "Method Not Allowed", "status": 405})
    );
}

async fn ping() -> &'static str {
    "pong"
}

fn router() -> Router {
    Router::new()
        .route("/ping", get(ping))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
}

#[tokio::test]
async fn an_unknown_route_gets_the_404_problem() {
    let request = Request::builder()
        .uri("/nowhere")
        .body(Body::empty())
        .unwrap();

    let response = router().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
}

#[tokio::test]
async fn an_unsupported_method_gets_the_405_problem_with_the_allow_header() {
    let request = Request::builder()
        .method(Method::DELETE)
        .uri("/ping")
        .body(Body::empty())
        .unwrap();

    let response = router().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    assert!(response.headers().contains_key(ALLOW));
}

async fn assert_internal_problem(response: axum::response::Response) {
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    assert_eq!(
        body_of(response).await,
        json!({"type": "about:blank", "title": "Internal Server Error", "status": 500})
    );
}

#[tokio::test]
async fn a_str_panic_is_the_500_problem_and_its_message_is_logged() {
    let (recorder, _guard) = LogRecorder::install();

    assert_internal_problem(panic_response(Box::new("boom"))).await;

    let lines = recorder.lines();
    assert_eq!(lines.len(), 1);
    let line = lines.first().unwrap();
    assert_eq!(line.level, Level::ERROR);
    assert_eq!(
        line.field("error"),
        Some("a request handler panicked: boom")
    );
}

#[tokio::test]
async fn a_string_panic_is_the_500_problem_and_its_message_is_logged() {
    let (recorder, _guard) = LogRecorder::install();

    assert_internal_problem(panic_response(Box::new("bang".to_owned()))).await;

    assert_eq!(
        recorder.lines().first().unwrap().field("error"),
        Some("a request handler panicked: bang")
    );
}

#[tokio::test]
async fn a_panic_with_another_payload_is_the_500_problem() {
    let (recorder, _guard) = LogRecorder::install();

    assert_internal_problem(panic_response(Box::new(42_u8))).await;

    assert_eq!(
        recorder.lines().first().unwrap().field("error"),
        Some("a request handler panicked: the panic payload is not text")
    );
}
