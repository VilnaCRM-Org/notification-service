use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{
    ALLOW, CACHE_CONTROL, CONNECTION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_SECURITY_POLICY,
    CONTENT_TYPE, TRANSFER_ENCODING, VARY,
};
use axum::http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode};
use axum::routing::{get, post};
use event_stream_application::graphql::{StreamLink, StreamPhase};
use event_stream_application::http::EventStreamState;
use event_stream_application::{
    Hub, HubSettings, ReplaySettings, StatusSource, StreamSettings, Subscriptions, TailState,
    Window,
};
use event_stream_domain::Cursor;
use futures_util::StreamExt as _;
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde_json::{Value, json};
use shared_infrastructure::bus::InMemoryQueryBus;
use test_support::log::{LogLine, LogRecorder};
use test_support::static_topic::StaticTopic;
use tokio::sync::watch;
use tower::ServiceExt as _;
use tracing::Level;

use super::*;
use crate::graphql::QueryStringLimit;
use crate::graphql::schema::{MAX_COMPLEXITY, builder};
use crate::http::security_headers;

/// The bound of every awaited call of the tests on a node's state.
const LIMIT: Duration = Duration::from_secs(5);

/// The one error of a subscription on the JSON transport (FR-5).
const TRANSPORT_ERROR: &str = "Subscriptions are not supported on this transport.";

/// The subscription of the JSON-transport tests.
const SUBSCRIPTION: &str = r#"subscription { events(types: ["a"]) { id } }"#;

fn router_of(schema: ApiSchema) -> Router {
    Router::new()
        .route("/api/graphql", post(execute).get(subscribe_or_explore))
        .with_state(schema)
        .layer(security_headers::layer())
}

fn router() -> Router {
    router_of(builder().finish())
}

async fn send(request: Request<Body>) -> Response<Body> {
    router().oneshot(request).await.unwrap()
}

/// The state of a started node: its hub has a position and takes up to four subscribers, so a
/// subscription whose resolver ran would register one.
fn started_state() -> EventStreamState {
    let state = unstarted_state();
    state.hub.start(Cursor::new(1));
    state
}

/// The state of a node whose hub has no position yet: a subscription is refused with the 503
/// problem `the node is starting`.
fn unstarted_state() -> EventStreamState {
    let hub = Hub::new(HubSettings {
        max_subscribers: 4,
        buffer: 8,
    });
    let window = Arc::new(Window::new(16, usize::MAX));
    let (_tail, tail_state) = watch::channel(TailState::default());
    let subscriptions = Subscriptions::new(
        Arc::clone(&hub),
        Arc::clone(&window),
        Arc::new(StaticTopic::empty()),
        tail_state.clone(),
        ReplaySettings {
            max_events: 1_000,
            max_data_bytes: 65_536,
            permits: 1,
        },
    );
    EventStreamState {
        query_bus: Arc::new(InMemoryQueryBus::new()),
        subscriptions: Arc::new(subscriptions),
        status: Arc::new(StatusSource::new(
            tail_state,
            window,
            Arc::<Hub>::clone(&hub),
        )),
        hub,
        settings: StreamSettings {
            keep_alive: Duration::from_secs(15),
        },
    }
}

/// `body` posted as a GraphQL request without an event-stream `Accept`, answered within
/// [`LIMIT`] by a router whose schema holds `state`.
async fn post_to(state: &EventStreamState, body: &Value) -> Response<Body> {
    let router = router_of(builder().data(state.clone()).finish());
    tokio::time::timeout(LIMIT, router.oneshot(post_json(&body.to_string())))
        .await
        .expect("the request answers in time")
        .unwrap()
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
#[case::first_operation("A", json!({ "a": { "name": "EventType" } }))]
#[case::second_operation("B", json!({ "b": "Query" }))]
#[tokio::test]
async fn variables_and_the_operation_name_are_honoured(
    #[case] operation_name: &str,
    #[case] data: Value,
) {
    let body = json!({
        "query": "query A($name: String!) { a: __type(name: $name) { name } } query B { b: __typename }",
        "operationName": operation_name,
        "variables": { "name": "EventType" },
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

#[tokio::test]
async fn a_subscription_on_the_json_transport_is_the_one_transport_error_and_registers_nothing() {
    let state = started_state();

    let response = post_to(&state, &json!({ "query": SUBSCRIPTION })).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), "application/json");
    assert_eq!(
        json_of(response).await,
        json!({ "data": null, "errors": [{ "message": TRANSPORT_ERROR }] })
    );
    assert_eq!(state.hub.subscriber_count(), 0);
    assert_eq!(state.status.snapshot().subscribers, 0);
}

/// The validation error of `subscription { __typename }`, which async-graphql reports twice.
const UNKNOWN_TYPENAME: &str = r#"Unknown field "__typename" on type "Subscription"."#;

#[rstest]
#[case::two_root_fields(
    r#"subscription { a: events(types: ["a"]) { id } b: events(types: ["b"]) { id } }"#,
    &[TRANSPORT_ERROR]
)]
#[case::a_root_fragment_spread(
    r#"subscription { ...F } fragment F on Subscription { events(types: ["a"]) { id } }"#,
    &[TRANSPORT_ERROR]
)]
#[case::the_same_response_name_twice(
    r#"subscription { events(types: ["a"]) { id } events(types: ["a"]) { type } }"#,
    &[TRANSPORT_ERROR]
)]
#[case::a_root_inline_fragment(
    r#"subscription { ... on Subscription { events(types: ["a"]) { id } } }"#,
    &[TRANSPORT_ERROR]
)]
#[case::a_skipped_root_field(
    r#"subscription { events(types: ["a"]) @skip(if: true) { id } }"#,
    &[TRANSPORT_ERROR]
)]
#[case::a_root_field_with_a_false_skip(
    r#"subscription { events(types: ["a"]) @skip(if: false) { id } }"#,
    &[TRANSPORT_ERROR]
)]
#[case::an_included_root_field(
    r#"subscription { events(types: ["a"]) @include(if: true) { id } }"#,
    &[TRANSPORT_ERROR]
)]
#[case::the_typename_next_to_events(
    r#"subscription { events(types: ["a"]) { id } __typename }"#,
    &[UNKNOWN_TYPENAME, UNKNOWN_TYPENAME]
)]
#[case::an_invalid_type_name(
    r#"subscription { events(types: ["A"]) { id } }"#,
    &[TRANSPORT_ERROR]
)]
#[case::the_typename_of_the_root(
    "subscription { __typename }",
    &[UNKNOWN_TYPENAME, UNKNOWN_TYPENAME]
)]
#[tokio::test]
async fn a_subscription_of_any_root_shape_keeps_its_json_answer(
    #[case] query: &str,
    #[case] expected: &[&str],
) {
    let state = started_state();

    let messages = refusal_of(post_to(&state, &json!({ "query": query })).await).await;

    assert_eq!(messages, expected);
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn the_node_of_the_json_transport_tests_takes_the_subscriber_of_a_streamed_subscription() {
    let state = started_state();
    let schema = builder().data(state.clone()).finish();
    let link = StreamLink::new(None);

    let mut responses =
        schema.execute_stream(async_graphql::Request::new(SUBSCRIPTION).data(link.clone()));
    tokio::time::timeout(LIMIT, async {
        tokio::select! {
            biased;
            item = responses.next() => panic!("a result before the release: {item:?}"),
            () = link.decided() => {}
        }
    })
    .await
    .expect("the resolver runs in time");

    assert_eq!(link.phase(), StreamPhase::Opened);
    assert_eq!(state.hub.subscriber_count(), 1);
    drop(responses);
    assert_eq!(state.hub.subscriber_count(), 0);
}

/// The messages of the errors of a GraphQL answer, whose `data` must be `null`.
async fn refusal_of(response: Response<Body>) -> Vec<String> {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), "application/json");
    let body = json_of(response).await;
    assert_eq!(body["data"], Value::Null, "{body}");
    body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| error["message"].as_str().unwrap().to_owned())
        .collect()
}

#[rstest]
#[case::two_aliases(r#"subscription { events(types: ["a"]) { a: data b: data } }"#)]
#[case::one_name_twice(r#"subscription { events(types: ["a"]) { data data } }"#)]
#[case::through_a_fragment(
    r#"subscription { events(types: ["a"]) { data ...F } } fragment F on Event { data }"#
)]
#[case::through_an_inline_fragment(
    r#"subscription { events(types: ["a"]) { data ... on Event { data } } }"#
)]
#[tokio::test]
async fn a_subscription_that_selects_the_event_data_twice_is_too_complex(#[case] query: &str) {
    let state = started_state();

    let messages = refusal_of(post_to(&state, &json!({ "query": query })).await).await;

    assert_eq!(messages, ["Query is too complex."]);
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn a_subscription_that_selects_every_event_field_once_passes_validation() {
    let state = started_state();
    let query = r#"subscription { events(types: ["a"]) { id type data } }"#;

    let messages = refusal_of(post_to(&state, &json!({ "query": query })).await).await;

    assert_eq!(messages, [TRANSPORT_ERROR]);
    assert_eq!(state.hub.subscriber_count(), 0);
}

/// A subscription of complexity `1 + 501 + ids`: the field `events`, its `data` once and `ids`
/// aliased selections of `id`, written compactly so that it stays within [`MAX_QUERY_CHARS`].
fn data_and_ids(ids: usize) -> String {
    let mut selection = String::from("data");
    for n in 0..ids {
        write!(selection, " i{n}:id").unwrap();
    }
    let document = format!(r#"subscription {{ events(types: ["a"]) {{ {selection} }} }}"#);
    assert!(document.chars().count() <= MAX_QUERY_CHARS);
    document
}

#[rstest]
#[case::at_the_limit(MAX_COMPLEXITY - 502, TRANSPORT_ERROR)]
#[case::one_above_the_limit(MAX_COMPLEXITY - 501, "Query is too complex.")]
#[tokio::test]
async fn the_event_data_costs_five_hundred_and_one_under_the_complexity_limit(
    #[case] ids: usize,
    #[case] message: &str,
) {
    let state = started_state();

    let response = post_to(&state, &json!({ "query": data_and_ids(ids) })).await;

    assert_eq!(refusal_of(response).await, [message]);
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

// ---- the transport of a POST (FR-2, FR-3, FR-5, FR-6, FR-7) ----

/// The body limit of the router of the transport tests.
const BODY_LIMIT: usize = 256;

/// The `Accept` fields that do not negotiate an event stream on a POST.
const JSON_ACCEPTS: [&str; 3] = [
    "*/*",
    "application/graphql-response+json, application/graphql+json, application/json, text/event-stream, multipart/mixed",
    "text/event-stream;q=0",
];

/// A whole answer of a body that ends.
#[derive(Debug, PartialEq, Eq)]
struct Answer {
    status: StatusCode,
    content_type: Option<String>,
    vary: Option<String>,
    body: String,
}

/// A GraphQL body above [`BODY_LIMIT`].
fn oversized() -> String {
    json!({ "query": "{ __typename }", "pad": "x".repeat(BODY_LIMIT) }).to_string()
}

/// A POST of `body` with the media type `content_type` and the request headers `headers`.
fn post_with(content_type: &str, body: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut request = post_body(content_type, body);
    for (name, value) in headers {
        request.headers_mut().append(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
    }
    request
}

/// The response of a router whose schema holds `state` and whose body limit is [`BODY_LIMIT`].
async fn respond_to(state: &EventStreamState, request: Request<Body>) -> Response<Body> {
    let router =
        router_of(builder().data(state.clone()).finish()).layer(DefaultBodyLimit::max(BODY_LIMIT));
    tokio::time::timeout(LIMIT, router.oneshot(request))
        .await
        .expect("the request answers in time")
        .unwrap()
}

fn header(headers: &HeaderMap, name: HeaderName) -> Option<String> {
    headers
        .get(name)
        .map(|value| value.to_str().unwrap().to_owned())
}

/// The whole answer of `request`, whose body must end.
async fn answer_of(state: &EventStreamState, request: Request<Body>) -> Answer {
    let response = respond_to(state, request).await;
    let (parts, body) = response.into_parts();
    let bytes = tokio::time::timeout(LIMIT, body.collect())
        .await
        .expect("the body ends in time")
        .unwrap()
        .to_bytes();
    Answer {
        status: parts.status,
        content_type: header(&parts.headers, CONTENT_TYPE),
        vary: header(&parts.headers, VARY),
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

/// The bodies of the transport tests, with the status a POST without `Accept` answers.
fn bodies() -> Vec<(&'static str, String, StatusCode)> {
    vec![
        (
            "application/json",
            json!({ "query": "{ eventStreamStatus { state subscribers } }" }).to_string(),
            StatusCode::OK,
        ),
        (
            "application/json",
            json!({ "query": SUBSCRIPTION }).to_string(),
            StatusCode::OK,
        ),
        (
            "text/plain",
            json!({ "query": "{ __typename }" }).to_string(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            "text/plain",
            oversized(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            "application/json",
            oversized(),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (
            "application/json",
            r#"{"query":"#.to_owned(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "application/json",
            "[]".to_owned(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "application/json",
            "{}".to_owned(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ]
}

#[tokio::test]
async fn a_post_that_does_not_negotiate_an_event_stream_keeps_its_answer() {
    let state = started_state();
    for (content_type, body, status) in bodies() {
        let reference = answer_of(&state, post_with(content_type, &body, &[])).await;
        assert_eq!(reference.status, status, "{content_type} {body}");
        assert_eq!(reference.vary, None, "{body}");
        for accept in JSON_ACCEPTS {
            let answer = answer_of(
                &state,
                post_with(content_type, &body, &[("accept", accept)]),
            )
            .await;
            assert_eq!(answer, reference, "{accept} {body}");
        }
    }
    let subscription = answer_of(
        &state,
        post_with(
            "application/json",
            &json!({ "query": SUBSCRIPTION }).to_string(),
            &[],
        ),
    )
    .await;
    assert_eq!(
        subscription.content_type.as_deref(),
        Some("application/json")
    );
    assert_eq!(
        serde_json::from_str::<Value>(&subscription.body).unwrap(),
        json!({ "data": null, "errors": [{ "message": TRANSPORT_ERROR }] })
    );
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn a_post_with_sse_whose_body_is_refused_gets_the_same_problem_and_no_stream() {
    let state = started_state();
    for (content_type, body, status) in bodies() {
        if status == StatusCode::OK {
            continue;
        }
        let reference = answer_of(&state, post_with(content_type, &body, &[])).await;
        let answer = answer_of(
            &state,
            post_with(content_type, &body, &[("accept", "text/event-stream")]),
        )
        .await;
        assert_eq!(answer, reference, "{content_type} {body}");
        assert_eq!(answer.status, status, "{body}");
        assert_eq!(
            answer.content_type.as_deref(),
            Some("application/problem+json")
        );
    }
    assert_eq!(state.hub.subscriber_count(), 0);
}

/// The subscription of the event-stream tests, which selects every field of `Event`.
const EVENTS: &str = r#"subscription { events(types: ["a"]) { id type data } }"#;

#[tokio::test(start_paused = true)]
async fn a_subscription_posted_with_sse_opens_the_event_stream() {
    let state = started_state();
    let body = json!({ "query": EVENTS }).to_string();

    let response = respond_to(
        &state,
        post_with(
            "application/json",
            &body,
            &[("accept", "text/event-stream")],
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert_eq!(headers[CONTENT_TYPE], "text/event-stream");
    assert_eq!(headers[CACHE_CONTROL], "no-cache");
    assert_eq!(headers["x-accel-buffering"], "no");
    assert_eq!(headers[VARY], "accept");
    assert_eq!(headers.get_all(VARY).iter().count(), 1);
    for name in [
        CONNECTION,
        CONTENT_ENCODING,
        CONTENT_LENGTH,
        TRANSFER_ENCODING,
    ] {
        assert!(!headers.contains_key(&name), "{name}");
    }
    assert_eq!(state.hub.subscriber_count(), 1);
    let mut body = response.into_body();
    let first = tokio::time::timeout(LIMIT, body.frame())
        .await
        .expect("the first chunk comes in time")
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    assert_eq!(first, ":\n\n");
    drop(body);
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn a_query_posted_with_sse_is_the_json_result_in_one_next_block_then_complete() {
    let state = started_state();
    let body = json!({ "query": "{ eventStreamStatus { state subscribers } }" }).to_string();
    let json = answer_of(&state, post_with("application/json", &body, &[])).await;

    let answer = answer_of(
        &state,
        post_with(
            "application/json",
            &body,
            &[("accept", "text/event-stream")],
        ),
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.content_type.as_deref(), Some("text/event-stream"));
    assert_eq!(answer.vary.as_deref(), Some("accept"));
    assert_eq!(
        answer.body,
        format!(
            ":\n\nevent: next\ndata: {}\n\nevent: complete\ndata:\n\n",
            json.body
        )
    );
}

#[tokio::test]
async fn the_last_event_id_header_of_a_post_is_the_cursor_of_its_subscription() {
    let state = started_state();
    let body = json!({ "query": EVENTS }).to_string();

    let answer = answer_of(
        &state,
        post_with(
            "application/json",
            &body,
            &[("accept", "text/event-stream"), ("last-event-id", "x")],
        ),
    )
    .await;

    let data = answer
        .body
        .strip_prefix(":\n\nevent: next\ndata: ")
        .and_then(|rest| rest.strip_suffix("\n\nevent: complete\ndata:\n\n"))
        .unwrap_or_else(|| panic!("{answer:?}"));
    let result: Value = serde_json::from_str(data).unwrap();
    assert_eq!(result["data"], Value::Null);
    assert_eq!(
        result["errors"][0]["extensions"]["violations"],
        json!([{
            "propertyPath": "Last-Event-ID",
            "message": "Last-Event-ID must be a decimal unsigned 64-bit integer",
            "code": "format",
        }])
    );
    assert_eq!(state.hub.subscriber_count(), 0);
}

/// Subscriptions that pass document validation and break the one-root-field rule (FR-14).
const ROOT_BREACHES: [&str; 5] = [
    r#"subscription { a: events(types: ["a"]) { id } b: events(types: ["b"]) { id } }"#,
    r#"subscription { ...F } fragment F on Subscription { events(types: ["a"]) { id } }"#,
    r#"subscription { ... on Subscription { events(types: ["a"]) { id } } }"#,
    r#"subscription { events(types: ["a"]) @skip(if: false) { id } }"#,
    r#"subscription { events(types: ["a"]) @include(if: true) { id } }"#,
];

/// The single result of an event-stream body that must be the comment, one `next` and
/// `complete`.
fn single_result_of(answer: &Answer) -> Value {
    let data = answer
        .body
        .strip_prefix(":\n\nevent: next\ndata: ")
        .and_then(|rest| rest.strip_suffix("\n\nevent: complete\ndata:\n\n"))
        .unwrap_or_else(|| panic!("{answer:?}"));
    serde_json::from_str(data).unwrap()
}

#[tokio::test]
async fn the_root_rule_runs_only_when_the_post_negotiates_an_event_stream() {
    let state = started_state();
    for query in ROOT_BREACHES {
        let body = json!({ "query": query }).to_string();
        let reference = answer_of(&state, post_with("application/json", &body, &[])).await;
        assert_eq!(reference.content_type.as_deref(), Some("application/json"));
        assert_eq!(
            serde_json::from_str::<Value>(&reference.body).unwrap(),
            json!({ "data": null, "errors": [{ "message": TRANSPORT_ERROR }] }),
            "{query}"
        );
        for accept in JSON_ACCEPTS {
            let answer = answer_of(
                &state,
                post_with("application/json", &body, &[("accept", accept)]),
            )
            .await;
            assert_eq!(answer, reference, "{accept} {query}");
        }

        let stream = answer_of(
            &state,
            post_with(
                "application/json",
                &body,
                &[("accept", "text/event-stream")],
            ),
        )
        .await;

        assert_eq!(stream.status, StatusCode::OK);
        assert_eq!(stream.content_type.as_deref(), Some("text/event-stream"));
        let result = single_result_of(&stream);
        assert_eq!(result["data"], Value::Null, "{query}");
        assert_eq!(result["errors"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            result["errors"][0]["extensions"],
            json!({
                "code": "VALIDATION_FAILED",
                "violations": [{
                    "propertyPath": "query",
                    "message": "a subscription operation must select exactly one root field",
                    "code": "single_root_field",
                }],
            }),
            "{query}"
        );
    }
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn the_typename_of_the_subscription_root_is_the_rule_error_only_on_the_event_stream() {
    let state = started_state();
    let body = json!({ "query": "subscription { __typename }" }).to_string();

    let json = answer_of(&state, post_with("application/json", &body, &[])).await;
    let stream = answer_of(
        &state,
        post_with(
            "application/json",
            &body,
            &[("accept", "text/event-stream")],
        ),
    )
    .await;

    let messages: Vec<Value> = serde_json::from_str::<Value>(&json.body).unwrap()["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| error["message"].clone())
        .collect();
    assert_eq!(messages, [UNKNOWN_TYPENAME, UNKNOWN_TYPENAME]);
    assert_eq!(
        single_result_of(&stream)["errors"][0]["extensions"]["violations"][0]["code"],
        "single_root_field"
    );
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn an_accept_field_alone_chooses_the_transport_of_the_same_request() {
    let state = started_state();
    let body = json!({ "query": "{ __typename }" }).to_string();

    let json = answer_of(
        &state,
        post_with("application/json", &body, &[("accept", "application/json")]),
    )
    .await;
    let stream = answer_of(
        &state,
        post_with(
            "application/json",
            &body,
            &[("accept", "application/json;q=0.5, text/event-stream")],
        ),
    )
    .await;

    assert_eq!(json.content_type.as_deref(), Some("application/json"));
    assert_eq!(json.body, r#"{"data":{"__typename":"Query"}}"#);
    assert_eq!(stream.content_type.as_deref(), Some("text/event-stream"));
    assert_eq!(
        stream.body,
        ":\n\nevent: next\ndata: {\"data\":{\"__typename\":\"Query\"}}\n\nevent: complete\ndata:\n\n"
    );
}

// ---- the GET form (FR-2, FR-4, FR-5, FR-6) ----

/// The query-string bound of the schemas of the GET tests.
const QUERY_LIMIT: usize = 512;

/// The body of today's 405 problem of a GET outside `dev`.
const METHOD_NOT_ALLOWED_BODY: &str =
    r#"{"type":"about:blank","title":"Method Not Allowed","status":405}"#;

/// The `Accept` fields with which a GET does not negotiate an event stream; `None` sends none.
const NON_STREAM_GET_ACCEPTS: [Option<&str>; 5] = [
    None,
    Some("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    Some("application/json"),
    Some("text/event-stream;q=0"),
    Some(
        "application/graphql-response+json, application/graphql+json, application/json, text/event-stream, multipart/mixed",
    ),
];

/// The GET handler a router serves: the one of `dev` or the one of every other `APP_ENV`.
#[derive(Debug, Clone, Copy)]
enum Mode {
    Dev,
    Other,
}

const MODES: [Mode; 2] = [Mode::Dev, Mode::Other];

/// A router whose GET of the path is the handler of `mode`, over `schema`.
fn get_router(schema: ApiSchema, mode: Mode) -> Router {
    let route = match mode {
        Mode::Dev => get(subscribe_or_explore),
        Mode::Other => get(subscribe),
    };
    Router::new()
        .route("/api/graphql", route)
        .with_state(schema)
        .layer(security_headers::layer())
}

/// The schema of a node with `state` and the query-string bound [`QUERY_LIMIT`].
fn limited(state: &EventStreamState) -> ApiSchema {
    builder()
        .data(state.clone())
        .data(QueryStringLimit {
            max_bytes: QUERY_LIMIT,
        })
        .finish()
}

/// `text` percent-encoded as a value of a query string: every byte but the unreserved ones.
fn encoded(text: &str) -> String {
    text.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// A GET of the path with `query_string` and the request headers `headers`.
fn get_with(query_string: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/graphql?{query_string}"))
        .body(Body::empty())
        .unwrap();
    for (name, value) in headers {
        request.headers_mut().append(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
    }
    request
}

/// The query string that sends `document` alone.
fn query_of(document: &str) -> String {
    format!("query={}", encoded(document))
}

/// The response of the router of `mode` over `schema` to `request`, within [`LIMIT`].
async fn get_response(schema: ApiSchema, mode: Mode, request: Request<Body>) -> Response<Body> {
    tokio::time::timeout(LIMIT, get_router(schema, mode).oneshot(request))
        .await
        .expect("the request answers in time")
        .unwrap()
}

/// The whole answer of `response`, whose body must end.
async fn whole(response: Response<Body>) -> Answer {
    let (parts, body) = response.into_parts();
    let bytes = tokio::time::timeout(LIMIT, body.collect())
        .await
        .expect("the body ends in time")
        .unwrap()
        .to_bytes();
    Answer {
        status: parts.status,
        content_type: header(&parts.headers, CONTENT_TYPE),
        vary: header(&parts.headers, VARY),
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

/// The whole answer of the router of `mode` over the limited schema of `state` to `request`.
async fn get_answer(state: &EventStreamState, mode: Mode, request: Request<Body>) -> Answer {
    whole(get_response(limited(state), mode, request).await).await
}

/// The first chunk of the event-stream `body`, within [`LIMIT`].
async fn first_chunk(body: &mut Body) -> String {
    let frame = tokio::time::timeout(LIMIT, body.frame())
        .await
        .expect("the first chunk comes in time")
        .unwrap()
        .unwrap();
    String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_subscription_sent_as_a_get_with_sse_opens_the_event_stream_in_every_mode() {
    for mode in MODES {
        let state = started_state();

        let response = get_response(
            limited(&state),
            mode,
            get_with(&query_of(EVENTS), &[("accept", "text/event-stream")]),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK, "{mode:?}");
        let headers = response.headers();
        assert_eq!(headers[CONTENT_TYPE], "text/event-stream", "{mode:?}");
        assert_eq!(headers[CACHE_CONTROL], "no-cache", "{mode:?}");
        assert_eq!(headers["x-accel-buffering"], "no", "{mode:?}");
        assert_eq!(headers[VARY], "accept", "{mode:?}");
        assert_eq!(state.hub.subscriber_count(), 1, "{mode:?}");
        let mut body = response.into_body();
        assert_eq!(first_chunk(&mut body).await, ":\n\n", "{mode:?}");
        drop(body);
        assert_eq!(state.hub.subscriber_count(), 0, "{mode:?}");
    }
}

#[tokio::test]
async fn a_query_sent_as_a_get_with_sse_is_the_result_of_the_same_post_in_one_next_block() {
    let state = started_state();
    let document =
        "query A($name: String!) { a: __type(name: $name) { name } } query B { b: __typename }";
    let post = json!({
        "query": document,
        "operationName": "A",
        "variables": { "name": "EventType" },
        "extensions": { "ignored": true },
    });
    let json = answer_of(
        &state,
        post_with("application/json", &post.to_string(), &[]),
    )
    .await;
    assert_eq!(json.body, r#"{"data":{"a":{"name":"EventType"}}}"#);
    let query_string = format!(
        "{}&operationName=A&variables={}&extensions={}&unknown=1",
        query_of(document),
        encoded(r#"{"name":"EventType"}"#),
        encoded(r#"{"ignored":true}"#),
    );

    for mode in MODES {
        let answer = get_answer(
            &state,
            mode,
            get_with(&query_string, &[("accept", "text/event-stream")]),
        )
        .await;

        assert_eq!(answer.status, StatusCode::OK, "{mode:?}");
        assert_eq!(answer.content_type.as_deref(), Some("text/event-stream"));
        assert_eq!(answer.vary.as_deref(), Some("accept"));
        assert_eq!(
            answer.body,
            format!(
                ":\n\nevent: next\ndata: {}\n\nevent: complete\ndata:\n\n",
                json.body
            ),
            "{mode:?}"
        );
    }
}

#[tokio::test]
async fn a_get_with_a_wildcard_accept_negotiates_the_event_stream() {
    let state = started_state();
    let query_string = query_of("{ __typename }");

    for (accept, mode) in [("*/*", Mode::Other), ("text/*", Mode::Dev)] {
        let answer = get_answer(&state, mode, get_with(&query_string, &[("accept", accept)])).await;

        assert_eq!(
            answer.body,
            ":\n\nevent: next\ndata: {\"data\":{\"__typename\":\"Query\"}}\n\nevent: complete\ndata:\n\n",
            "{accept}"
        );
    }
}

#[tokio::test]
async fn a_get_that_does_not_negotiate_sse_keeps_its_answer_whatever_its_query_string() {
    let state = started_state();
    let (explorer_response, page) = explorer_page().await;
    let explorer_policy = explorer_response.headers()[CONTENT_SECURITY_POLICY].clone();
    for query_string in [
        query_of(EVENTS),
        "%zz".to_owned(),
        String::new(),
        "query=a&query=b".to_owned(),
    ] {
        for accept in NON_STREAM_GET_ACCEPTS {
            let headers: Vec<(&str, &str)> =
                accept.iter().map(|value| ("accept", *value)).collect();

            let refused = get_response(
                limited(&state),
                Mode::Other,
                get_with(&query_string, &headers),
            )
            .await;
            assert_eq!(
                refused.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{accept:?}"
            );
            assert_eq!(refused.headers()[ALLOW], "POST", "{accept:?}");
            let refused = whole(refused).await;
            assert_eq!(
                refused.content_type.as_deref(),
                Some("application/problem+json")
            );
            assert_eq!(refused.body, METHOD_NOT_ALLOWED_BODY, "{accept:?}");

            let explored = get_response(
                limited(&state),
                Mode::Dev,
                get_with(&query_string, &headers),
            )
            .await;
            assert_eq!(explored.status(), StatusCode::OK, "{accept:?}");
            assert_eq!(
                explored.headers()[CONTENT_SECURITY_POLICY],
                explorer_policy,
                "{accept:?}"
            );
            assert_eq!(whole(explored).await.body, page, "{accept:?}");
        }
    }
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn the_last_event_id_header_of_a_get_is_the_cursor_of_its_subscription() {
    let state = started_state();
    let document = r#"subscription { events(types: ["a"], lastEventId: "5") { id } }"#;

    let answer = get_answer(
        &state,
        Mode::Other,
        get_with(
            &query_of(document),
            &[("accept", "text/event-stream"), ("last-event-id", "x")],
        ),
    )
    .await;

    assert_eq!(
        single_result_of(&answer)["errors"][0]["extensions"]["violations"],
        json!([{
            "propertyPath": "Last-Event-ID",
            "message": "Last-Event-ID must be a decimal unsigned 64-bit integer",
            "code": "format",
        }])
    );
    assert_eq!(state.hub.subscriber_count(), 0);
}

/// The problem document of `answer`, which must be a problem.
fn problem_body(answer: &Answer) -> Value {
    assert_eq!(
        answer.content_type.as_deref(),
        Some("application/problem+json"),
        "{answer:?}"
    );
    serde_json::from_str(&answer.body).unwrap()
}

#[tokio::test]
async fn a_refused_query_string_is_its_problem_and_no_stream() {
    let state = started_state();
    let too_long = format!("query={}", "a".repeat(QUERY_LIMIT));
    let refusals = [
        (
            too_long.as_str(),
            StatusCode::URI_TOO_LONG,
            "The query string is too long.",
        ),
        (
            "query=%zz",
            StatusCode::BAD_REQUEST,
            "Failed to deserialize query string: invalid percent-encoding or UTF-8 in `query`",
        ),
        (
            "operationName=A",
            StatusCode::BAD_REQUEST,
            "Failed to deserialize query string: missing field `query`",
        ),
        (
            "query=%7B__typename%7D&variables=1",
            StatusCode::UNPROCESSABLE_ENTITY,
            "The request failed validation.",
        ),
    ];
    for (query_string, status, detail) in refusals {
        for mode in MODES {
            let answer = get_answer(
                &state,
                mode,
                get_with(query_string, &[("accept", "text/event-stream")]),
            )
            .await;

            assert_eq!(answer.status, status, "{mode:?} {query_string}");
            assert_eq!(
                problem_body(&answer)["detail"],
                detail,
                "{mode:?} {query_string}"
            );
            assert_eq!(answer.vary, None);
        }
    }
    assert_eq!(state.hub.subscriber_count(), 0);
}

/// The 422 of the GET form answers before the rules of FR-6 steps 3 to 6: a document error, a
/// breach of the root rule, invalid arguments, the 503 of a node that cannot take the subscriber
/// and the stream itself.
#[tokio::test]
async fn the_422_of_the_get_form_answers_before_the_document_and_the_node() {
    let unstarted = unstarted_state();
    let documents = [
        "{ unknownField }",
        r#"subscription { a: events(types: ["a"]) { id } b: events(types: ["b"]) { id } }"#,
        r#"subscription { events(types: ["Not A Type"]) { id } }"#,
        EVENTS,
    ];
    for state in [started_state(), unstarted] {
        for document in documents {
            let query_string = format!("{}&variables=%5B%5D", query_of(document));

            let answer = get_answer(
                &state,
                Mode::Other,
                get_with(&query_string, &[("accept", "text/event-stream")]),
            )
            .await;

            assert_eq!(
                answer.status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{document}"
            );
            assert_eq!(
                problem_body(&answer)["violations"],
                json!([{
                    "propertyPath": "variables",
                    "message": "variables must be a JSON-encoded object",
                    "code": "format",
                }]),
                "{document}"
            );
        }
        assert_eq!(state.hub.subscriber_count(), 0);
    }
}

#[tokio::test]
async fn a_get_with_sse_on_a_schema_without_the_query_string_limit_is_the_500_problem_logged_once()
{
    let state = started_state();
    for mode in MODES {
        for query_string in [query_of(EVENTS), "%zz".to_owned()] {
            let schema = builder().data(state.clone()).finish();
            let (recorder, guard) = LogRecorder::install();

            let answer = whole(
                get_response(
                    schema,
                    mode,
                    get_with(&query_string, &[("accept", "text/event-stream")]),
                )
                .await,
            )
            .await;
            drop(guard);

            assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR, "{mode:?}");
            assert_eq!(
                problem_body(&answer),
                json!({"type": "about:blank", "title": "Internal Server Error", "status": 500})
            );
            assert_eq!(
                recorder.lines(),
                vec![LogLine {
                    level: Level::ERROR,
                    target: "shared_application::http::problem".to_owned(),
                    message: "request failed".to_owned(),
                    fields: vec![(
                        "error".to_owned(),
                        "the GraphQL schema has no QueryStringLimit data for the GET form"
                            .to_owned()
                    )],
                }],
                "{mode:?} {query_string}"
            );
        }
    }
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn a_get_without_sse_on_a_schema_without_the_query_string_limit_keeps_its_answer() {
    let state = started_state();
    let (recorder, guard) = LogRecorder::install();

    let refused = whole(
        get_response(
            builder().data(state.clone()).finish(),
            Mode::Other,
            get_with(&query_of(EVENTS), &[]),
        )
        .await,
    )
    .await;
    let explored = whole(
        get_response(
            builder().data(state.clone()).finish(),
            Mode::Dev,
            get_with(&query_of(EVENTS), &[]),
        )
        .await,
    )
    .await;
    drop(guard);

    assert_eq!(refused.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(refused.body, METHOD_NOT_ALLOWED_BODY);
    assert_eq!(explored.status, StatusCode::OK);
    assert_eq!(
        explored.content_type.as_deref(),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
}

// ---- the bounds of the document and the operation name (FR-29) ----

/// A character that is one code point and four bytes of UTF-8, so a string of it is counted in
/// characters and not in bytes.
const WIDE: &str = "\u{1F600}";

/// The query-string bound of the schemas of the bound tests: the default of
/// `HTTP_MAX_BODY_BYTES`, so only the bounds of the members can refuse their requests.
const WIDE_QUERY_LIMIT: usize = 65_536;

/// The violations of the 422 problem of a request whose members `(name, bound)` are longer than
/// their bounds, one `length` violation each, in the order of the problem.
fn length_violations(members: &[(&str, usize)]) -> Value {
    members
        .iter()
        .map(|(name, bound)| {
            json!({
                "propertyPath": name,
                "message": format!("{name} must contain at most {bound} characters"),
                "code": "length",
            })
        })
        .collect()
}

/// The bodies whose `query` or `operationName`, or both, are longer than their bounds, with the
/// violations of their 422 problem.
fn overlong_bodies() -> Vec<(Value, Value)> {
    let query_at_bound = WIDE.repeat(MAX_QUERY_CHARS);
    let name_at_bound = WIDE.repeat(MAX_OPERATION_NAME_CHARS);
    vec![
        (
            json!({ "query": "a".repeat(MAX_QUERY_CHARS + 1) }),
            length_violations(&[("query", MAX_QUERY_CHARS)]),
        ),
        (
            json!({ "query": format!("{query_at_bound}{WIDE}"), "operationName": name_at_bound }),
            length_violations(&[("query", MAX_QUERY_CHARS)]),
        ),
        (
            json!({
                "query": query_at_bound,
                "operationName": format!("{name_at_bound}{WIDE}"),
            }),
            length_violations(&[("operationName", MAX_OPERATION_NAME_CHARS)]),
        ),
        (
            json!({
                "query": "a".repeat(MAX_QUERY_CHARS + 1),
                "operationName": "A".repeat(MAX_OPERATION_NAME_CHARS + 1),
                "variables": {},
            }),
            length_violations(&[
                ("operationName", MAX_OPERATION_NAME_CHARS),
                ("query", MAX_QUERY_CHARS),
            ]),
        ),
    ]
}

/// The query string of the GET form that carries the members of `body`.
fn query_string_of(body: &Value) -> String {
    body.as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| {
            let text = value
                .as_str()
                .map_or_else(|| value.to_string(), ToOwned::to_owned);
            format!("{name}={}", encoded(&text))
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// The schema of a node with `state` and the query-string bound [`WIDE_QUERY_LIMIT`].
fn widely_limited(state: &EventStreamState) -> ApiSchema {
    builder()
        .data(state.clone())
        .data(QueryStringLimit {
            max_bytes: WIDE_QUERY_LIMIT,
        })
        .finish()
}

#[tokio::test]
async fn a_post_whose_document_or_operation_name_is_too_long_is_the_422_problem_on_both_transports()
{
    let state = started_state();
    for (body, violations) in overlong_bodies() {
        for accept in [None, Some("text/event-stream")] {
            let headers: Vec<(&str, &str)> =
                accept.iter().map(|value| ("accept", *value)).collect();
            let router = router_of(builder().data(state.clone()).finish());

            let response = router
                .oneshot(post_with("application/json", &body.to_string(), &headers))
                .await
                .unwrap();

            assert_eq!(
                response.status(),
                StatusCode::UNPROCESSABLE_ENTITY,
                "{accept:?}"
            );
            assert_eq!(content_type(&response), "application/problem+json");
            let problem = json_of(response).await;
            assert_eq!(problem["type"], "/problems/validation-failed");
            assert_eq!(problem["violations"], violations, "{accept:?}");
        }
    }
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn a_get_whose_document_or_operation_name_is_too_long_is_the_422_problem_in_every_mode() {
    let state = started_state();
    for (body, violations) in overlong_bodies() {
        for mode in MODES {
            let answer = whole(
                get_response(
                    widely_limited(&state),
                    mode,
                    get_with(&query_string_of(&body), &[("accept", "text/event-stream")]),
                )
                .await,
            )
            .await;

            assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY, "{mode:?}");
            assert_eq!(problem_body(&answer)["violations"], violations, "{mode:?}");
        }
    }
    assert_eq!(state.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn the_format_violation_of_the_get_form_comes_before_the_length_violations() {
    let state = started_state();
    let query_string = format!("query={}&variables=1", "a".repeat(MAX_QUERY_CHARS + 1));

    let answer = whole(
        get_response(
            widely_limited(&state),
            Mode::Other,
            get_with(&query_string, &[("accept", "text/event-stream")]),
        )
        .await,
    )
    .await;

    assert_eq!(
        problem_body(&answer)["violations"],
        json!([{
            "propertyPath": "variables",
            "message": "variables must be a JSON-encoded object",
            "code": "format",
        }])
    );
}

#[tokio::test]
async fn a_document_and_an_operation_name_at_their_bounds_are_executed_on_both_forms() {
    let state = started_state();
    let operation = "A".repeat(MAX_OPERATION_NAME_CHARS);
    let document = format!(
        "query {operation} {{ __typename }} #{}",
        WIDE.repeat(MAX_QUERY_CHARS - MAX_OPERATION_NAME_CHARS - 23)
    );
    assert_eq!(document.chars().count(), MAX_QUERY_CHARS);
    let body = json!({ "query": document, "operationName": operation });

    let posted = post_to(&state, &body).await;
    assert_eq!(posted.status(), StatusCode::OK);
    assert_eq!(
        json_of(posted).await,
        json!({ "data": { "__typename": "Query" } })
    );
    for mode in MODES {
        let answer = whole(
            get_response(
                widely_limited(&state),
                mode,
                get_with(&query_string_of(&body), &[("accept", "text/event-stream")]),
            )
            .await,
        )
        .await;

        assert_eq!(answer.status, StatusCode::OK, "{mode:?}");
        assert_eq!(
            single_result_of(&answer),
            json!({ "data": { "__typename": "Query" } })
        );
    }
}

/// The entry of the document for `method` of the `GraphQL` path.
fn entry(method: &str) -> Value {
    serde_json::to_value(crate::openapi::document()).unwrap()["paths"]["/api/graphql"][method]
        .clone()
}

/// The sorted keys of `object`.
fn keys(object: &Value) -> Vec<String> {
    let mut keys: Vec<String> = object.as_object().unwrap().keys().cloned().collect();
    keys.sort_unstable();
    keys
}

#[test]
fn the_post_entry_documents_the_body_both_transports_and_the_problems() {
    let post = entry("post");

    assert_eq!(post["operationId"], "api_graphql_post");
    assert_eq!(post["tags"], json!(["GraphQL"]));
    let body = &post["requestBody"]["content"]["application/json"];
    assert_eq!(
        body["schema"],
        json!({"$ref": "#/components/schemas/GraphQLRequest"})
    );
    assert_eq!(
        body["examples"]["query"]["value"],
        json!({"query": "{ eventTypes { name } }"})
    );
    let subscription = body["examples"]["subscription"]["value"]["query"]
        .as_str()
        .unwrap();
    assert!(subscription.contains(r#"events(types: ["load.test"])"#));
    let ok = &post["responses"]["200"]["content"];
    assert_eq!(
        ok["application/json"]["schema"],
        json!({"$ref": "#/components/schemas/GraphQLResponse"})
    );
    assert_eq!(
        ok["text/event-stream"]["schema"],
        json!({"$ref": "#/components/schemas/GraphQLEvent"})
    );
    assert_eq!(
        keys(&ok["text/event-stream"]["examples"]),
        ["complete", "next"]
    );
    assert_eq!(
        keys(&post["responses"]),
        ["200", "400", "413", "415", "422", "503", "default"]
    );
    assert_eq!(post["parameters"][0]["name"], "Last-Event-ID");
}

#[test]
fn the_get_entry_documents_the_query_string_both_answers_and_the_problems() {
    let get = entry("get");

    assert_eq!(get["operationId"], "api_graphql_get");
    assert_eq!(get["tags"], json!(["GraphQL"]));
    let parameters: Vec<(&str, bool)> = get["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| {
            (
                parameter["name"].as_str().unwrap(),
                parameter["required"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        parameters,
        [
            ("query", true),
            ("operationName", false),
            ("variables", false),
            ("extensions", false),
            ("Last-Event-ID", false),
        ]
    );
    for index in [2, 3] {
        let schema = &get["parameters"][index]["schema"];
        assert_eq!(schema["contentMediaType"], "application/json");
        assert_eq!(schema["contentSchema"], json!({"type": "object"}));
    }
    assert!(
        get["parameters"][0]["example"]
            .as_str()
            .unwrap()
            .contains(r#"events(types: ["load.test"])"#)
    );
    let ok = &get["responses"]["200"]["content"];
    assert_eq!(keys(ok), ["text/event-stream", "text/html"]);
    assert_eq!(
        ok["text/event-stream"]["schema"],
        json!({"$ref": "#/components/schemas/GraphQLEvent"})
    );
    assert_eq!(
        keys(&get["responses"]),
        ["200", "400", "405", "414", "422", "503", "default"]
    );
}

#[test]
fn both_get_handlers_carry_the_same_operation() {
    use utoipa::Path as _;

    assert_eq!(
        serde_json::to_value(__path_subscribe::operation()).unwrap(),
        serde_json::to_value(__path_subscribe_or_explore::operation()).unwrap()
    );
    assert_eq!(__path_subscribe::path(), GRAPHQL_PATH);
    assert_eq!(__path_execute::path(), GRAPHQL_PATH);
}
