use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use http_body_util::BodyExt as _;
use serde_json::json;
use tower::ServiceExt as _;

use super::request_timeout;

async fn slow() -> &'static str {
    tokio::time::sleep(Duration::from_secs(1)).await;
    "late"
}

async fn quick() -> &'static str {
    tokio::time::sleep(Duration::from_millis(10)).await;
    "on time"
}

fn router() -> Router {
    Router::new()
        .route("/slow", get(slow))
        .route("/quick", get(quick))
        .layer(from_fn_with_state(
            Duration::from_millis(50),
            request_timeout,
        ))
}

async fn send(uri: &str) -> axum::response::Response {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    router().oneshot(request).await.unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_request_over_budget_gets_the_503_problem() {
    let response = send("/slow").await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        json!({
            "type": "about:blank",
            "title": "Service Unavailable",
            "status": 503,
            "detail": "The request exceeded its time budget.",
        })
    );
}

#[tokio::test(start_paused = true)]
async fn a_request_within_budget_is_answered_by_the_handler() {
    let response = send("/quick").await;

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(bytes, "on time");
}
