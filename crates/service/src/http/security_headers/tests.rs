use axum::Router;
use axum::body::Body;
use axum::http::header::CONTENT_SECURITY_POLICY;
use axum::http::{HeaderValue, Request, Response};
use axum::routing::get;
use tower::ServiceExt as _;

use super::*;

async fn get_response(path: &str) -> Response<Body> {
    Router::new()
        .route("/plain", get(|| std::future::ready("ok")))
        .route(
            "/page",
            get(|| {
                std::future::ready((
                    [(
                        CONTENT_SECURITY_POLICY,
                        "default-src 'self'; frame-ancestors 'none'",
                    )],
                    "page",
                ))
            }),
        )
        .layer(layer())
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn header<'a>(response: &'a Response<Body>, name: &str) -> &'a HeaderValue {
    response.headers().get(name).unwrap()
}

#[tokio::test]
async fn sets_the_four_headers_when_the_handler_sets_none() {
    let response = get_response("/plain").await;

    assert_eq!(header(&response, "x-content-type-options"), "nosniff");
    assert_eq!(header(&response, "x-frame-options"), "DENY");
    assert_eq!(header(&response, "referrer-policy"), "no-referrer");
    assert_eq!(
        header(&response, "content-security-policy"),
        "default-src 'none'; frame-ancestors 'none'"
    );
}

#[tokio::test]
async fn keeps_a_policy_the_handler_supplies() {
    let response = get_response("/page").await;

    assert_eq!(
        response
            .headers()
            .get_all("content-security-policy")
            .iter()
            .count(),
        1
    );
    assert_eq!(
        header(&response, "content-security-policy"),
        "default-src 'self'; frame-ancestors 'none'"
    );
    assert_eq!(header(&response, "x-frame-options"), "DENY");
}

#[tokio::test]
async fn sets_the_headers_on_an_unmatched_path_too() {
    let response = get_response("/missing").await;

    assert_eq!(response.status(), 404);
    assert_eq!(header(&response, "x-content-type-options"), "nosniff");
}
