use axum::Router;
use axum::body::Body;
use axum::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE};
use axum::http::{Method, Request, Response, StatusCode};
use axum::routing::post;
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde_json::{Value, json};
use tower::ServiceExt as _;

use super::*;
use crate::graphql::schema::builder;
use crate::http::security_headers;

fn router() -> Router {
    Router::new()
        .route("/api/graphql", post(execute).get(explorer))
        .with_state(builder().finish())
        .layer(security_headers::layer())
}

async fn send(request: Request<Body>) -> Response<Body> {
    router().oneshot(request).await.unwrap()
}

fn post_body(content_type: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/api/graphql")
        .header(CONTENT_TYPE, content_type)
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn post_json(body: &str) -> Request<Body> {
    post_body("application/json", body)
}

async fn json_of(response: Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn content_type(response: &Response<Body>) -> &str {
    response.headers()[CONTENT_TYPE].to_str().unwrap()
}

#[tokio::test]
async fn a_valid_operation_is_200_json() {
    let response = send(post_json(r#"{"query":"{ __typename }"}"#)).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), "application/json");
    assert_eq!(
        json_of(response).await,
        json!({ "data": { "__typename": "Query" } })
    );
}

#[rstest]
#[case::first_operation("A", json!({ "a": { "name": "Mutation" } }))]
#[case::second_operation("B", json!({ "b": "Query" }))]
#[tokio::test]
async fn variables_and_the_operation_name_are_honoured(
    #[case] operation_name: &str,
    #[case] data: Value,
) {
    let body = json!({
        "query": "query A($name: String!) { a: __type(name: $name) { name } } query B { b: __typename }",
        "operationName": operation_name,
        "variables": { "name": "Mutation" },
    });

    let response = send(post_json(&body.to_string())).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_of(response).await, json!({ "data": data }));
}

#[rstest]
#[case::empty_object("{}")]
#[case::array("[]")]
#[case::unrelated_member(r#"{"foo":1}"#)]
#[case::query_is_a_number(r#"{"query":5}"#)]
#[case::variables_are_a_number(r#"{"query":"{ __typename }","variables":5}"#)]
#[tokio::test]
async fn a_body_that_is_not_a_graphql_request_is_the_422_problem(#[case] body: &str) {
    let response = send(post_json(body)).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(content_type(&response), "application/problem+json");
    let problem = json_of(response).await;
    assert_eq!(problem["type"], "about:blank");
    assert_eq!(problem["status"], 422);
}

#[rstest]
#[case::empty_object("{}")]
#[case::unrelated_member(r#"{"foo":1}"#)]
#[case::query_is_a_number(r#"{"query":5}"#)]
#[case::variables_are_a_number(r#"{"query":"{ __typename }","variables":5}"#)]
#[tokio::test]
async fn the_object_bodies_are_rejected_with_the_documented_detail(#[case] body: &str) {
    let problem = json_of(send(post_json(body)).await).await;

    assert_eq!(
        problem["detail"],
        "The request body is not a GraphQL request."
    );
}

#[tokio::test]
async fn malformed_json_is_the_400_problem() {
    let response = send(post_json(r#"{"query":"#)).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(content_type(&response), "application/problem+json");
    assert_eq!(json_of(response).await["type"], "about:blank");
}

#[tokio::test]
async fn the_wrong_media_type_is_the_415_problem() {
    let response = send(post_body("text/plain", r#"{"query":"{ __typename }"}"#)).await;

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(content_type(&response), "application/problem+json");
    assert_eq!(json_of(response).await["status"], 415);
}

#[tokio::test]
async fn a_document_that_is_not_valid_for_the_schema_is_200_with_errors() {
    let response = send(post_json(r#"{"query":"{ doesNotExist }"}"#)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["errors"].as_array().unwrap().len(), 1);
    assert!(body.get("data").is_none() || body["data"].is_null());
}

#[tokio::test]
async fn a_syntax_error_is_200_with_errors() {
    let response = send(post_json(r#"{"query":"{ "}"#)).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(json_of(response).await["errors"].is_array());
}

/// The attributes of every `script` and `link` element of the page, as `(name, value)` lists.
fn elements(page: &str) -> Vec<Vec<(String, String)>> {
    page.split('<')
        .filter(|part| part.starts_with("script") || part.starts_with("link"))
        .map(|part| {
            let tag = part.split('>').next().unwrap();
            tag.split('"')
                .collect::<Vec<_>>()
                .chunks(2)
                .filter_map(|pair| match pair {
                    [name, value] => Some((
                        name.rsplit(' ')
                            .next()
                            .unwrap()
                            .trim_end_matches('=')
                            .to_owned(),
                        (*value).to_owned(),
                    )),
                    _ => None,
                })
                .collect()
        })
        .collect()
}

fn attribute<'a>(element: &'a [(String, String)], name: &str) -> Option<&'a str> {
    element
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// Whether `url` contains `@<major>.<minor>.<patch>/`.
fn is_pinned(url: &str) -> bool {
    url.split('@').skip(1).any(|rest| {
        let version = rest.split('/').next().unwrap();
        let numbers: Vec<&str> = version.split('.').collect();
        rest.contains('/')
            && numbers.len() == 3
            && numbers
                .iter()
                .all(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
    })
}

async fn explorer_page() -> (Response<Body>, String) {
    let response = send(Request::get("/api/graphql").body(Body::empty()).unwrap()).await;
    let (parts, body) = response.into_parts();
    let page = String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap();
    (Response::from_parts(parts, Body::empty()), page)
}

#[tokio::test]
async fn the_explorer_page_is_html_with_its_own_policy() {
    let (response, page) = explorer_page().await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), "text/html; charset=utf-8");
    assert_eq!(
        response
            .headers()
            .get_all(CONTENT_SECURITY_POLICY)
            .iter()
            .count(),
        1
    );
    assert_eq!(
        response.headers()[CONTENT_SECURITY_POLICY],
        EXPLORER_CONTENT_SECURITY_POLICY
    );
    assert!(page.contains("GraphiQL.createFetcher({ url: '/api/graphql' })"));
}

#[tokio::test]
async fn every_asset_of_the_explorer_is_pinned_and_has_an_integrity_digest() {
    let (_response, page) = explorer_page().await;

    let assets: Vec<Vec<(String, String)>> = elements(&page)
        .into_iter()
        .filter(|element| {
            attribute(element, "src").is_some() || attribute(element, "href").is_some()
        })
        .collect();
    let urls: Vec<&str> = assets
        .iter()
        .map(|element| {
            attribute(element, "src")
                .or_else(|| attribute(element, "href"))
                .unwrap()
        })
        .collect();
    assert_eq!(
        urls,
        [
            "https://unpkg.com/graphiql@3.9.0/graphiql.min.css",
            "https://unpkg.com/react@18.3.1/umd/react.production.min.js",
            "https://unpkg.com/react-dom@18.3.1/umd/react-dom.production.min.js",
            "https://unpkg.com/graphiql@3.9.0/graphiql.min.js",
        ]
    );
    for (element, url) in assets.iter().zip(&urls) {
        assert!(url.starts_with("https://unpkg.com/"), "{url}");
        assert!(is_pinned(url), "{url} has no exact version");
        assert!(
            attribute(element, "integrity").is_some_and(|digest| digest.starts_with("sha384-")),
            "{url} has no integrity attribute"
        );
        assert_eq!(
            attribute(element, "crossorigin"),
            Some("anonymous"),
            "{url}"
        );
    }
    let digests: Vec<&str> = assets
        .iter()
        .map(|element| attribute(element, "integrity").unwrap())
        .collect();
    assert_eq!(
        digests,
        [
            "sha384-QMux00XgRtwRLSYIY3kw2rj1ovk5AuuliAchk+HSQbqdbGFnz9GYuqIlOqxhwCE2",
            "sha384-DGyLxAyjq0f9SPpVevD6IgztCFlnMF6oW/XQGmfe+IsZ8TqEiDrcHkMLKI6fiB/Z",
            "sha384-gTGxhz21lVGYNMcdJOyq01Edg0jhn/c22nsx0kyqP0TxaV5WVdsSH1fSDUf5YJj1",
            "sha384-8NGfVj4CVlqHajlZj+bPJT4thxPMHMJYn7DWK2CvtopLd02E7qsPHazniBYvVjOO",
        ]
    );
}

#[tokio::test]
async fn the_explorer_page_has_no_other_link_and_no_favicon() {
    let (_response, page) = explorer_page().await;

    assert_eq!(page.matches("https://").count(), 4);
    assert!(!page.contains("favicon"));
    assert!(!page.contains("rel=\"icon\""));
}

#[test]
fn the_explorer_policy_names_exactly_the_one_origin() {
    let origins: Vec<&str> = EXPLORER_CONTENT_SECURITY_POLICY
        .split([' ', ';'])
        .filter(|token| token.contains("://"))
        .collect();

    assert_eq!(origins, ["https://unpkg.com", "https://unpkg.com"]);
    assert!(EXPLORER_CONTENT_SECURITY_POLICY.ends_with("frame-ancestors 'none'"));
    assert!(EXPLORER_CONTENT_SECURITY_POLICY.contains("connect-src 'self'"));
}

#[test]
fn the_pin_check_accepts_exact_versions_only() {
    assert!(is_pinned("https://unpkg.com/react@18.3.1/umd/react.js"));
    assert!(!is_pinned("https://unpkg.com/react@18/umd/react.js"));
    assert!(!is_pinned("https://unpkg.com/react@18.3/umd/react.js"));
    assert!(!is_pinned("https://unpkg.com/react@latest/umd/react.js"));
    assert!(!is_pinned("https://unpkg.com/react/umd/react.js"));
    assert!(!is_pinned("https://unpkg.com/react@18.3.1"));
}
