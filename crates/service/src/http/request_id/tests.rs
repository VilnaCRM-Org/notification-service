use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request};
use axum::middleware::from_fn;
use axum::routing::get;
use rstest::rstest;
use shared_domain::Uuid;
use tower::{ServiceBuilder, ServiceExt as _};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

use super::*;

fn router() -> Router {
    Router::new()
        .route("/", get(|| std::future::ready("ok")))
        .layer(
            ServiceBuilder::new()
                .layer(from_fn(sanitize_request_id))
                .layer(SetRequestIdLayer::new(REQUEST_ID_HEADER, MakeRequestUuid))
                .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER)),
        )
}

async fn echoed(value: Option<&[u8]>) -> String {
    let mut request = Request::get("/").body(Body::empty()).unwrap();
    if let Some(value) = value {
        request
            .headers_mut()
            .insert(REQUEST_ID_HEADER, HeaderValue::from_bytes(value).unwrap());
    }
    let response = router().oneshot(request).await.unwrap();
    response
        .headers()
        .get(REQUEST_ID_HEADER)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

#[rstest]
#[case::mixed(b"abc-123_x.y".to_vec())]
#[case::longest(vec![b'a'; 128])]
#[case::one_character(b"7".to_vec())]
#[tokio::test]
async fn echoes_an_acceptable_id_unchanged(#[case] value: Vec<u8>) {
    assert_eq!(echoed(Some(&value)).await.as_bytes(), value.as_slice());
}

#[rstest]
#[case::too_long(vec![b'a'; 129])]
#[case::space_and_bang(b"bad value!".to_vec())]
#[case::empty(Vec::new())]
#[case::slash(b"a/b".to_vec())]
#[case::non_ascii("\u{e9}t\u{e9}".as_bytes().to_vec())]
#[tokio::test]
async fn replaces_an_unacceptable_id_with_a_uuid(#[case] value: Vec<u8>) {
    let id = echoed(Some(&value)).await;

    assert_ne!(id.as_bytes(), value.as_slice());
    assert!(Uuid::parse(&id).is_ok(), "{id} is not a UUID");
}

#[tokio::test]
async fn generates_a_uuid_when_the_header_is_absent() {
    let id = echoed(None).await;

    assert!(Uuid::parse(&id).is_ok(), "{id} is not a UUID");
}

#[test]
fn accepts_exactly_the_documented_pattern() {
    assert!(is_acceptable(b"A.z-0_9"));
    assert!(is_acceptable(&[b'a'; 128]));
    assert!(!is_acceptable(b""));
    assert!(!is_acceptable(&[b'a'; 129]));
    assert!(!is_acceptable(b"a b"));
    assert!(!is_acceptable(b"a:b"));
    assert!(!is_acceptable(b"a{b"));
    assert!(!is_acceptable(b"a`b"));
    assert!(!is_acceptable(b"a@b"));
    assert!(!is_acceptable(b"a[b"));
}
