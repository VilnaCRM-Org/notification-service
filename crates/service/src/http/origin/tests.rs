use std::time::Duration as StdDuration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use http_body_util::BodyExt as _;
use rstest::rstest;
use tower::{ServiceBuilder, ServiceExt as _};

use super::*;

const APP: &str = "https://app.example.com";
const OTHER: &str = "https://other.example.com";
/// The `Sec-WebSocket-Key` of the test handshakes: sixteen zero bytes in base64, a valid key that
/// no secrets scanner takes for a credential.
const HANDSHAKE_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAA==";

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

fn list(origins: &[&str]) -> AllowedOrigins {
    AllowedOrigins::List(
        origins
            .iter()
            .map(|origin| HeaderValue::from_str(origin).unwrap())
            .collect(),
    )
}

/// A router with one route behind the two origin layers, in the order of the service router.
fn app(origins: &AllowedOrigins) -> Router {
    let layers = ServiceBuilder::new()
        .option_layer(cors(origins))
        .layer(from_fn_with_state(
            Arc::new(origins.clone()),
            websocket_origin,
        ));
    Router::new().route("/x", get(async || "ok")).layer(layers)
}

async fn send(app: &Router, request: Request<Body>) -> Answer {
    tokio::time::timeout(StdDuration::from_secs(5), async {
        let response = app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Answer {
            status: parts.status,
            headers: parts.headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    })
    .await
    .unwrap()
}

fn handshake(origin: Option<&str>) -> Request<Body> {
    let mut builder = Request::get("/x").header(SEC_WEBSOCKET_KEY, HANDSHAKE_KEY);
    if let Some(origin) = origin {
        builder = builder.header(ORIGIN, origin);
    }
    builder.body(Body::empty()).unwrap()
}

fn plain(origin: Option<&str>) -> Request<Body> {
    let mut builder = Request::get("/x");
    if let Some(origin) = origin {
        builder = builder.header(ORIGIN, origin);
    }
    builder.body(Body::empty()).unwrap()
}

fn preflight(path: &str, origin: &str) -> Request<Body> {
    Request::options(path)
        .header(ORIGIN, origin)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .body(Body::empty())
        .unwrap()
}

fn assert_refused(answer: &Answer) {
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(answer.headers[CONTENT_TYPE], "application/problem+json");
    assert_eq!(
        answer.body,
        r#"{"type":"about:blank","title":"Forbidden","status":403,"detail":"the origin is not allowed"}"#
    );
}

fn assert_passed(answer: &Answer) {
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, "ok");
}

#[test]
fn the_preflight_age_is_ten_minutes() {
    assert_eq!(PREFLIGHT_MAX_AGE, StdDuration::from_secs(600));
    assert_eq!(ORIGIN_NOT_ALLOWED, "the origin is not allowed");
}

#[test]
fn no_allowed_origin_means_no_cors_layer() {
    assert!(cors(&AllowedOrigins::NoOrigin).is_none());
    assert!(cors(&AllowedOrigins::Any).is_some());
    assert!(cors(&list(&[APP])).is_some());
}

#[tokio::test]
async fn without_an_allowed_origin_no_cors_header_is_added() {
    let app = app(&AllowedOrigins::NoOrigin);

    for request in [plain(Some(APP)), plain(None)] {
        let answer = send(&app, request).await;
        assert_passed(&answer);
        assert!(
            answer
                .headers
                .keys()
                .all(|name| !name.as_str().starts_with("access-control-")),
            "{:?}",
            answer.headers
        );
        assert!(!answer.headers.contains_key(header::VARY));
    }
    let options = send(&app, preflight("/x", APP)).await;
    assert_eq!(options.status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(
        !options
            .headers
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
}

#[tokio::test]
async fn a_listed_origin_gets_the_cors_headers_and_another_does_not() {
    let app = app(&list(&[APP, "http://localhost:3000"]));

    let allowed = send(&app, plain(Some(APP))).await;
    assert_passed(&allowed);
    assert_eq!(allowed.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], APP);
    assert!(
        allowed.headers[header::VARY]
            .to_str()
            .unwrap()
            .contains("origin")
    );
    let second = send(&app, plain(Some("http://localhost:3000"))).await;
    assert_eq!(
        second.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "http://localhost:3000"
    );

    let other = send(&app, plain(Some(OTHER))).await;
    assert_passed(&other);
    assert!(
        !other
            .headers
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );

    let without = send(&app, plain(None)).await;
    assert_passed(&without);
    assert!(
        !without
            .headers
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
}

#[tokio::test]
async fn every_origin_gets_the_wildcard_with_any() {
    let app = app(&AllowedOrigins::Any);

    let answer = send(&app, plain(Some(OTHER))).await;

    assert_passed(&answer);
    assert_eq!(answer.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
}

#[rstest]
#[case::list_allowed(list(&[APP]), APP, Some(APP))]
#[case::list_other(list(&[APP]), OTHER, None)]
#[case::any(AllowedOrigins::Any, OTHER, Some("*"))]
#[tokio::test]
async fn a_preflight_is_answered_for_a_known_and_an_unknown_path(
    #[case] origins: AllowedOrigins,
    #[case] origin: &str,
    #[case] allow_origin: Option<&str>,
) {
    let app = app(&origins);

    for path in ["/x", "/unknown"] {
        let answer = send(&app, preflight(path, origin)).await;

        assert_eq!(answer.status, StatusCode::OK, "{path}");
        assert_eq!(answer.body, "", "{path}");
        let methods = answer.headers[header::ACCESS_CONTROL_ALLOW_METHODS]
            .to_str()
            .unwrap()
            .to_owned();
        assert_eq!(methods, "GET,POST", "{path}");
        let headers = answer.headers[header::ACCESS_CONTROL_ALLOW_HEADERS]
            .to_str()
            .unwrap()
            .to_owned();
        assert_eq!(headers, "content-type,last-event-id", "{path}");
        assert_eq!(
            answer.headers[header::ACCESS_CONTROL_MAX_AGE],
            "600",
            "{path}"
        );
        assert!(
            !answer
                .headers
                .contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
        );
        assert!(
            !answer
                .headers
                .contains_key(header::ACCESS_CONTROL_EXPOSE_HEADERS)
        );
        assert_eq!(
            answer
                .headers
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .map(|value| value.to_str().unwrap()),
            allow_origin,
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_handshake_with_an_origin_is_refused_when_no_origin_is_allowed() {
    let app = app(&AllowedOrigins::NoOrigin);

    assert_refused(&send(&app, handshake(Some(APP))).await);
    assert_passed(&send(&app, handshake(None)).await);
    assert_passed(&send(&app, plain(Some(APP))).await);
}

#[tokio::test]
async fn a_handshake_passes_only_from_a_listed_origin() {
    let app = app(&list(&[APP]));

    assert_passed(&send(&app, handshake(Some(APP))).await);
    let refused = send(&app, handshake(Some(OTHER))).await;
    assert_refused(&refused);
    assert!(
        !refused
            .headers
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    // `vary` comes from the CORS layer, so the 403 carries it only while that layer wraps the
    // origin check of the handshake.
    assert!(
        refused.headers[header::VARY]
            .to_str()
            .unwrap()
            .contains("origin")
    );
    assert_passed(&send(&app, handshake(None)).await);
    assert_passed(&send(&app, plain(Some(OTHER))).await);
}

#[tokio::test]
async fn a_handshake_passes_from_every_origin_with_any() {
    let app = app(&AllowedOrigins::Any);

    assert_passed(&send(&app, handshake(Some(OTHER))).await);
    assert_passed(&send(&app, handshake(None)).await);
}
