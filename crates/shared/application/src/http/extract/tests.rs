use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, StatusCode};
use axum::routing::{get, post};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde::Deserialize;
use serde_json::{Value, json};
use test_support::log::LogRecorder;
use tower::ServiceExt as _;
use validator::Validate;

use super::{ApiJson, ApiPath, ValidatedJson, ValidatedQuery};

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Payload {
    #[validate(length(min = 1, message = "The name is empty."))]
    display_name: String,
    #[validate(range(min = 1, message = "The count is below 1."))]
    item_count: i32,
}

#[derive(Debug, Deserialize, Validate)]
struct Params {
    #[validate(range(min = 1, message = "The page is below 1."))]
    page: i32,
}

async fn api_json(ApiJson(payload): ApiJson<Payload>) -> String {
    payload.display_name
}

async fn validated_json(ValidatedJson(payload): ValidatedJson<Payload>) -> String {
    payload.display_name
}

async fn api_path(ApiPath(id): ApiPath<u32>) -> String {
    id.to_string()
}

async fn validated_query(ValidatedQuery(params): ValidatedQuery<Params>) -> String {
    params.page.to_string()
}

fn router() -> Router {
    Router::new()
        .route("/api-json", post(api_json))
        .route("/validated-json", post(validated_json))
        .route("/path/{id}", get(api_path))
        .route("/without-parameter", get(api_path))
        .route("/query", get(validated_query))
        .layer(DefaultBodyLimit::max(64))
}

struct Reply {
    status: StatusCode,
    content_type: String,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

async fn send(request: Request<Body>) -> Reply {
    let response = router().oneshot(request).await.unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        content_type,
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

fn post_json(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn assert_problem(reply: &Reply, status: StatusCode, problem_type: &str) {
    assert_eq!(reply.status, status);
    assert_eq!(reply.content_type, "application/problem+json");
    let body = reply.json();
    assert_eq!(body["type"], problem_type);
    assert_eq!(body["status"], status.as_u16());
}

#[rstest]
#[case("/api-json")]
#[case("/validated-json")]
#[tokio::test]
async fn a_valid_body_reaches_the_handler(#[case] uri: &str) {
    let reply = send(post_json(uri, r#"{"displayName":"x","itemCount":1}"#)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, "x");
}

#[rstest]
#[case("/api-json")]
#[case("/validated-json")]
#[tokio::test]
async fn malformed_json_is_a_400_problem(#[case] uri: &str) {
    let reply = send(post_json(uri, "{not json")).await;

    assert_problem(&reply, StatusCode::BAD_REQUEST, "about:blank");
}

#[rstest]
#[case("/api-json")]
#[case("/validated-json")]
#[tokio::test]
async fn a_body_without_the_json_media_type_is_a_415_problem(#[case] uri: &str) {
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .body(Body::from(r#"{"displayName":"x","itemCount":1}"#))
        .unwrap();

    let reply = send(request).await;

    assert_problem(&reply, StatusCode::UNSUPPORTED_MEDIA_TYPE, "about:blank");
}

#[rstest]
#[case("/api-json")]
#[case("/validated-json")]
#[tokio::test]
async fn an_unknown_field_is_a_422_about_blank_problem(#[case] uri: &str) {
    let reply = send(post_json(uri, r#"{"displayName":"x","itemCount":1,"x":1}"#)).await;

    assert_problem(&reply, StatusCode::UNPROCESSABLE_ENTITY, "about:blank");
}

#[tokio::test]
async fn a_body_that_is_not_valid_is_a_422_validation_problem_in_stable_order() {
    let reply = send(post_json(
        "/validated-json",
        r#"{"displayName":"","itemCount":0}"#,
    ))
    .await;

    assert_problem(
        &reply,
        StatusCode::UNPROCESSABLE_ENTITY,
        "/problems/validation-failed",
    );
    assert_eq!(
        reply.json()["violations"],
        json!([
            {"propertyPath": "displayName", "message": "The name is empty.", "code": "length"},
            {"propertyPath": "itemCount", "message": "The count is below 1.", "code": "range"},
        ])
    );
}

#[rstest]
#[case("/api-json")]
#[case("/validated-json")]
#[tokio::test]
async fn a_body_over_the_limit_is_a_413_problem(#[case] uri: &str) {
    let body = format!(r#"{{"displayName":"{}","itemCount":1}}"#, "x".repeat(100));

    let reply = send(post_json(uri, &body)).await;

    assert_problem(&reply, StatusCode::PAYLOAD_TOO_LARGE, "about:blank");
}

#[tokio::test]
async fn a_path_parameter_is_parsed() {
    let reply = send(get_request("/path/7")).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, "7");
}

#[tokio::test]
async fn a_path_parameter_of_the_wrong_type_is_a_400_problem() {
    let reply = send(get_request("/path/abc")).await;

    assert_problem(&reply, StatusCode::BAD_REQUEST, "about:blank");
}

#[tokio::test]
async fn a_route_without_the_parameter_is_an_internal_problem_without_detail() {
    let (recorder, _guard) = LogRecorder::install();

    let reply = send(get_request("/without-parameter")).await;

    assert_problem(&reply, StatusCode::INTERNAL_SERVER_ERROR, "about:blank");
    assert!(reply.json().get("detail").is_none());
    assert_eq!(recorder.lines().len(), 1);
}

#[tokio::test]
async fn a_valid_query_reaches_the_handler() {
    let reply = send(get_request("/query?page=3")).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, "3");
}

#[rstest]
#[case("/query?page=abc")]
#[case("/query?page=1.5")]
#[case("/query?page=4294967296")]
#[case("/query")]
#[tokio::test]
async fn a_query_that_cannot_be_parsed_is_a_400_problem(#[case] uri: &str) {
    let reply = send(get_request(uri)).await;

    assert_problem(&reply, StatusCode::BAD_REQUEST, "about:blank");
}

#[tokio::test]
async fn a_query_out_of_range_is_a_422_validation_problem() {
    let reply = send(get_request("/query?page=-1")).await;

    assert_problem(
        &reply,
        StatusCode::UNPROCESSABLE_ENTITY,
        "/problems/validation-failed",
    );
    assert_eq!(
        reply.json()["violations"],
        json!([{"propertyPath": "page", "message": "The page is below 1.", "code": "range"}])
    );
}
