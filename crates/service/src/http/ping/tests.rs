use axum::Router;
use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

use super::*;

#[tokio::test]
async fn answers_pong_as_plain_text() {
    let router: Router = Router::new().route("/ping", get(ping));

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
