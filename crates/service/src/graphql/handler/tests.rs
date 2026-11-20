use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{
    CACHE_CONTROL, CONNECTION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_SECURITY_POLICY,
    CONTENT_TYPE, TRANSFER_ENCODING, VARY,
};
use axum::http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode};
use axum::routing::post;
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
use test_support::static_topic::StaticTopic;
use tokio::sync::watch;
use tower::ServiceExt as _;

use super::*;
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
        .route("/api/graphql", post(execute).get(explorer))
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
    let hub = Hub::new(HubSettings {
        max_subscribers: 4,
        buffer: 8,
    });
    hub.start(Cursor::new(1));
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
/// aliased selections of `id`.
fn data_and_ids(ids: usize) -> String {
    let mut selection = String::from("data");
    for n in 0..ids {
        write!(selection, " i{n}: id").unwrap();
    }
    format!(r#"subscription {{ events(types: ["a"]) {{ {selection} }} }}"#)
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
