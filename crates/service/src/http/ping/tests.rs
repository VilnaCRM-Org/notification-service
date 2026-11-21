use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt as _;
use serde_json::json;
use tower::ServiceExt as _;

use crate::openapi::{document, operational_router};

#[tokio::test]
async fn answers_pong_as_plain_text() {
    let (router, _document) = operational_router().split_for_parts();

    let response = router
        .oneshot(Request::get("/ping").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"pong");
}

#[test]
fn the_entry_answers_pong_as_plain_text_under_health_check() {
    let entry = serde_json::to_value(document()).unwrap()["paths"]["/ping"]["get"].clone();

    assert_eq!(entry["operationId"], "ping_get");
    assert_eq!(entry["tags"], json!(["HealthCheck"]));
    assert!(entry.get("parameters").is_none());
    assert_eq!(
        entry["responses"]["200"]["content"],
        json!({"text/plain": {"schema": {"type": "string"}, "example": "pong"}})
    );
}
