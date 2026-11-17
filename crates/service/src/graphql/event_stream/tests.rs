use std::fmt::Write as _;
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use async_graphql::Request;
use axum::body::Body;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, VARY};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use event_stream_application::http::EventStreamState;
use event_stream_application::{
    EventTypeView, Hub, HubSettings, ListEventTypes, ReplaySettings, StatusSource, StreamSettings,
    Subscriptions, TailState, Window,
};
use event_stream_domain::{Cursor, Sequence, TopicReplay};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde_json::{Value, json};
use test_support::bus::FakeQueryBus;
use test_support::fixture::stored_event;
use test_support::log::{LogLine, LogRecorder};
use test_support::static_topic::StaticTopic;
use test_support::topic::ScriptedTopicReplay;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing::Level;

use super::{COMPLETE, KEEP_ALIVE, next_block, respond};
use crate::graphql::schema::{ApiSchema, MAX_COMPLEXITY, MAX_DEPTH, builder};

/// The bound of every awaited call of these tests; the clock is paused where a wait must pass.
const WAIT: Duration = Duration::from_secs(5);

/// The hub position and the window floor of a started node.
const POSITION: u64 = 1_000;

/// The replay bounds of the node: a cap no test reaches, one permit.
const REPLAY: ReplaySettings = ReplaySettings {
    max_events: 1_000_000,
    max_data_bytes: 65_536,
    permits: 1,
};

/// A subscription of the type `a` that selects every field of `Event`.
const EVENTS: &str = r#"subscription { events(types: ["a"]) { id type data } }"#;

/// The event object of the event `id`, as the producer wrote it: its members are not sorted.
fn event_json(id: u64, event_type: &str) -> String {
    format!(
        r#"{{"type":"{event_type}","id":"00000000-0000-0000-0000-{id:012x}","occurredAt":"2026-11-14T10:00:00Z","data":{{"z":{id},"a":"x"}}}}"#
    )
}

/// A node, started or not, its schema over the state and the sender of its tail state.
struct Node {
    state: EventStreamState,
    window: Arc<Window>,
    schema: ApiSchema,
    _tail: watch::Sender<TailState>,
}

impl Node {
    /// A node whose hub takes `max_subscribers` and whose replays read `replay`; `started` gives
    /// the hub its position [`POSITION`].
    fn new(started: bool, max_subscribers: usize, replay: Arc<dyn TopicReplay>) -> Self {
        let hub = Hub::new(HubSettings {
            max_subscribers,
            buffer: 8,
        });
        if started {
            hub.start(Cursor::new(POSITION));
        }
        let window = Arc::new(Window::new(16, usize::MAX));
        window.restart(Cursor::new(POSITION));
        let (tail, receiver) = watch::channel(TailState::default());
        let subscriptions = Subscriptions::new(
            Arc::clone(&hub),
            Arc::clone(&window),
            replay,
            receiver.clone(),
            REPLAY,
        );
        let catalog = FakeQueryBus::answering(|_query: ListEventTypes| {
            Ok(vec![EventTypeView {
                name: "a".to_owned(),
            }])
        });
        let state = EventStreamState {
            query_bus: Arc::new(catalog),
            subscriptions: Arc::new(subscriptions),
            status: Arc::new(StatusSource::new(
                receiver,
                Arc::clone(&window),
                Arc::<Hub>::clone(&hub),
            )),
            hub,
            settings: StreamSettings {
                keep_alive: Duration::from_secs(15),
            },
        };
        Self {
            schema: builder().data(state.clone()).finish(),
            state,
            window,
            _tail: tail,
        }
    }

    /// A started node with room for four subscribers, whose replays read an empty topic.
    fn ready() -> Self {
        Self::new(true, 4, Arc::new(StaticTopic::empty()))
    }

    /// What the tail does with a valid event: the window first, then the hub.
    fn pass(&self, id: u64, event_type: &str) {
        let event = stored_event(id, event_type, &event_json(id, event_type));
        self.window.push(Arc::new(event.clone()));
        self.state.hub.deliver(&event);
    }

    fn subscribers(&self) -> usize {
        self.state.hub.subscriber_count()
    }

    /// The answer of `respond` to `request` with the `Last-Event-ID` header `header`.
    async fn answer_with(&self, request: Request, header: Option<&str>) -> Response {
        let header = header.map(|text| HeaderValue::from_str(text).unwrap());
        timeout(WAIT, respond(&self.schema, request, header))
            .await
            .expect("respond answers in time")
    }

    /// The answer of `respond` to `document` without a header.
    async fn answer(&self, document: &str) -> Response {
        self.answer_with(Request::new(document), None).await
    }

    /// The JSON transport's result for `request`, compactly encoded.
    async fn json_transport(&self, request: Request) -> String {
        let response = timeout(WAIT, self.schema.execute(request))
            .await
            .expect("the request answers in time");
        serde_json::to_string(&response).unwrap()
    }
}

/// The next chunk of `body` within [`WAIT`]; `None` at its end.
async fn chunk(body: &mut Body) -> Option<String> {
    timeout(WAIT, body.frame())
        .await
        .expect("the body answers in time")
        .map(|frame| String::from_utf8(frame.unwrap().into_data().unwrap().to_vec()).unwrap())
}

/// True when `body` writes nothing while [`WAIT`] passes.
async fn quiet(body: &mut Body) -> bool {
    timeout(WAIT, body.frame()).await.is_err()
}

/// Every chunk of a body that ends.
async fn chunks(response: Response) -> Vec<String> {
    let mut body = response.into_body();
    let mut written = Vec::new();
    while let Some(text) = chunk(&mut body).await {
        written.push(text);
    }
    written
}

/// The names of the headers of `response`, sorted.
fn header_names(response: &Response) -> Vec<&str> {
    let mut names: Vec<&str> = response.headers().keys().map(HeaderName::as_str).collect();
    names.sort_unstable();
    names
}

/// The head of every event stream: 200 and exactly its four headers, each once (FR-7).
fn assert_event_stream_head(response: &Response) {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header_names(response),
        ["cache-control", "content-type", "vary", "x-accel-buffering"]
    );
    assert_eq!(response.headers().len(), 4);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
    assert_eq!(response.headers()[CACHE_CONTROL], "no-cache");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    assert_eq!(response.headers()[VARY], "accept");
}

/// The subscription result block of the event `id` of type `a` with every field selected.
fn event_block(id: u64) -> String {
    format!(
        "event: next\nid: {id}\ndata: {{\"data\":{{\"events\":{{\"id\":\"{id}\",\"type\":\"a\",\"data\":{}}}}}}}\n\n",
        event_json(id, "a")
    )
}

/// The single-result body of a result encoded as `data`.
fn single(data: &str) -> Vec<String> {
    vec![
        ":\n\n".to_owned(),
        format!("event: next\ndata: {data}\n\n"),
        "event: complete\ndata:\n\n".to_owned(),
    ]
}

/// The result of a single-result body, parsed: the body must be the comment, one `next` without
/// an `id:` line and `complete`.
fn single_result(written: &[String]) -> Value {
    assert_eq!(written.len(), 3, "{written:?}");
    assert_eq!(written[0], ":\n\n");
    assert_eq!(written[2], "event: complete\ndata:\n\n");
    let data = written[1]
        .strip_prefix("event: next\ndata: ")
        .and_then(|rest| rest.strip_suffix("\n\n"))
        .unwrap_or_else(|| panic!("{written:?}"));
    assert!(!data.contains('\n'), "{data:?}");
    serde_json::from_str(data).unwrap()
}

/// Only the four blocks of FR-8 are written: no other event name and no `retry:` field.
fn assert_only_protocol_blocks(written: &[String]) {
    for block in written {
        assert!(block.ends_with("\n\n"), "{block:?}");
        assert!(!block.contains("retry:"), "{block:?}");
        let events: Vec<&str> = block
            .lines()
            .filter_map(|line| line.strip_prefix("event: "))
            .collect();
        assert!(
            events.is_empty() || events == ["next"] || events == ["complete"],
            "{block:?}"
        );
    }
}

/// Polls `future` once: true when it is still pending.
async fn still_pending<F: Future>(mut future: Pin<&mut F>) -> bool {
    poll_fn(|context| Poll::Ready(future.as_mut().poll(context).is_pending())).await
}

fn lines_of(recorder: &LogRecorder, message: &str) -> Vec<LogLine> {
    recorder
        .lines()
        .into_iter()
        .filter(|line| line.message == message)
        .collect()
}

// ---- the wire blocks (FR-8) ----

#[test]
fn the_comment_and_complete_blocks_are_written_byte_for_byte() {
    assert_eq!(KEEP_ALIVE, ":\n\n");
    assert_eq!(COMPLETE, "event: complete\ndata:\n\n");
}

#[test]
fn a_result_block_has_an_id_line_only_with_an_id() {
    let result = async_graphql::Response::new(
        async_graphql::Value::from_json(json!({"events": {"type": "a"}})).unwrap(),
    );

    assert_eq!(
        next_block(&result, None),
        "event: next\ndata: {\"data\":{\"events\":{\"type\":\"a\"}}}\n\n"
    );
    assert_eq!(
        next_block(&result, Some(Sequence::new(1043).unwrap())),
        "event: next\nid: 1043\ndata: {\"data\":{\"events\":{\"type\":\"a\"}}}\n\n"
    );
    assert_eq!(
        next_block(&result, Some(Sequence::new(u64::MAX).unwrap())),
        "event: next\nid: 18446744073709551615\ndata: {\"data\":{\"events\":{\"type\":\"a\"}}}\n\n"
    );
}

#[test]
fn an_error_result_block_holds_null_data_and_the_errors() {
    let result =
        async_graphql::Response::from_errors(vec![async_graphql::ServerError::new("boom", None)]);

    assert_eq!(
        next_block(&result, None),
        "event: next\ndata: {\"data\":null,\"errors\":[{\"message\":\"boom\"}]}\n\n"
    );
}

#[rstest]
#[case::line_breaks("a\nb\r\nc\rd")]
#[case::unicode_line_separators("a\u{2028}b\u{2029}c")]
#[case::quotes_and_backslashes("say \"hi\" c:\\dir")]
#[case::control_characters("a\u{0}b\u{1f}c\td")]
#[case::outside_the_basic_multilingual_plane("\u{1F600} \u{1D11E} \u{10FFFF}")]
fn a_result_is_written_on_one_data_line_that_parses_back_unchanged(#[case] text: &str) {
    let result = async_graphql::Response::new(
        async_graphql::Value::from_json(json!({"text": text, "list": [text, {"inner": text}]}))
            .unwrap(),
    );

    let block = next_block(&result, Some(Sequence::new(7).unwrap()));

    let data = block
        .strip_prefix("event: next\nid: 7\ndata: ")
        .and_then(|rest| rest.strip_suffix("\n\n"))
        .unwrap_or_else(|| panic!("{block:?}"));
    assert!(!data.contains(['\n', '\r']), "{data:?}");
    assert_eq!(
        serde_json::from_str::<Value>(data).unwrap(),
        serde_json::to_value(&result).unwrap()
    );
}

// ---- a subscription that registers (FR-7, FR-9 (a), FR-10, FR-17) ----

#[tokio::test(start_paused = true)]
async fn a_registered_subscription_gets_the_head_then_one_comment_without_an_id() {
    let node = Node::ready();

    let response = node.answer(EVENTS).await;

    assert_event_stream_head(&response);
    assert_eq!(node.subscribers(), 1);
    assert_eq!(node.state.status.snapshot().subscribers, 1);
    let mut body = response.into_body();
    assert_eq!(chunk(&mut body).await.as_deref(), Some(":\n\n"));
    assert!(quiet(&mut body).await);
    drop(body);
    assert_eq!(node.subscribers(), 0);
}

#[rstest]
#[case::every_field(EVENTS, json!({"id": "1043", "type": "a", "data": "EVENT"}))]
#[case::the_id_aliased(
    r#"subscription { events(types: ["a"]) { eventId: id type } }"#,
    json!({"eventId": "1043", "type": "a"})
)]
#[case::the_id_not_selected(
    r#"subscription { events(types: ["a"]) { type } }"#,
    json!({"type": "a"})
)]
#[case::the_field_aliased(
    r#"subscription { feed: events(types: ["a"]) { type id } }"#,
    json!({"type": "a", "id": "1043"})
)]
#[tokio::test(start_paused = true)]
async fn an_event_of_the_subscribed_type_is_one_next_block_with_its_id_whatever_the_selection(
    #[case] document: &str,
    #[case] mut object: Value,
) {
    let node = Node::ready();
    let response = node.answer(document).await;
    let mut body = response.into_body();
    assert_eq!(chunk(&mut body).await.as_deref(), Some(":\n\n"));

    node.pass(1_042, "b");
    node.pass(1_043, "a");

    if object.get("data").is_some() {
        object["data"] = serde_json::from_str(&event_json(1_043, "a")).unwrap();
    }
    let name = if document.contains("feed:") {
        "feed"
    } else {
        "events"
    };
    let block = chunk(&mut body).await.unwrap();
    let data = block
        .strip_prefix("event: next\nid: 1043\ndata: ")
        .and_then(|rest| rest.strip_suffix("\n\n"))
        .unwrap_or_else(|| panic!("{block:?}"));
    assert!(!data.contains('\n'), "{data:?}");
    assert_eq!(
        serde_json::from_str::<Value>(data).unwrap(),
        json!({"data": {name: object}})
    );
    assert!(quiet(&mut body).await);
    assert_only_protocol_blocks(&[block]);
}

#[tokio::test(start_paused = true)]
async fn the_event_object_is_written_byte_for_byte_in_the_producers_member_order() {
    let node = Node::ready();
    let mut body = node.answer(EVENTS).await.into_body();
    assert_eq!(chunk(&mut body).await.as_deref(), Some(":\n\n"));

    node.pass(1_043, "a");
    node.pass(1_044, "a");

    assert_eq!(chunk(&mut body).await, Some(event_block(1_043)));
    assert_eq!(chunk(&mut body).await, Some(event_block(1_044)));
    assert!(quiet(&mut body).await);
}

#[tokio::test(start_paused = true)]
async fn a_cursor_below_the_window_gets_its_head_and_its_comment_before_any_broker_read() {
    let replay = ScriptedTopicReplay::new();
    let node = Node::new(true, 4, Arc::new(replay.clone()));

    let response = node
        .answer(r#"subscription { events(types: ["a"], lastEventId: "10") { id } }"#)
        .await;

    assert_event_stream_head(&response);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
    assert_eq!(node.subscribers(), 1);
    let mut body = response.into_body();
    assert_eq!(chunk(&mut body).await.as_deref(), Some(":\n\n"));
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
    assert!(quiet(&mut body).await);
    assert_eq!(replay.calls().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn the_header_cursor_reaches_the_subscription_and_wins_over_the_argument() {
    let node = Node::ready();
    node.pass(1_001, "a");
    node.pass(1_002, "a");
    node.pass(1_003, "a");
    let request = Request::new(
        r#"subscription { events(types: ["a"], lastEventId: "1001") { id type data } }"#,
    );

    let response = node.answer_with(request, Some("1002")).await;

    assert_event_stream_head(&response);
    let mut body = response.into_body();
    assert_eq!(chunk(&mut body).await.as_deref(), Some(":\n\n"));
    assert_eq!(chunk(&mut body).await, Some(event_block(1_003)));
    assert!(quiet(&mut body).await);
}

// ---- single results (FR-9 (b), FR-13, FR-15, FR-20) ----

/// A query nested `levels` fields deep through `ofType`.
fn nested(levels: usize) -> String {
    let mut document = String::from(r#"{ __type(name: "Query") { fields { type "#);
    for _ in 0..levels {
        document.push_str("{ ofType ");
    }
    document.push_str("{ name }");
    for _ in 0..levels {
        document.push_str(" }");
    }
    document.push_str(" } } }");
    document
}

/// A query of complexity `complexity`: the field `eventTypes` and `complexity - 1` aliased
/// selections of its `name`.
fn of_complexity(complexity: usize) -> String {
    let mut document = String::from("{ eventTypes {");
    for n in 1..complexity {
        write!(document, " n{n}: name").unwrap();
    }
    document.push_str(" } }");
    document
}

#[rstest]
#[case::a_query("{ eventTypes { name } }")]
#[case::the_status_query("{ eventStreamStatus { state subscribers } }")]
#[case::an_introspection_query("{ __schema { queryType { name } subscriptionType { name } } }")]
#[case::a_mutation("mutation { eventTypes { name } }")]
#[case::a_document_that_does_not_parse("{ eventTypes { name }")]
#[case::an_unknown_field("{ unknownField }")]
#[case::a_field_error("{ eventTypes(page: 0) { name } }")]
#[case::two_operations_without_a_name("query A { __typename } query B { __typename }")]
#[tokio::test]
async fn a_query_or_a_document_error_is_one_next_block_then_complete_like_the_json_transport(
    #[case] document: &str,
) {
    let node = Node::ready();
    let expected = node.json_transport(Request::new(document)).await;

    let response = node.answer(document).await;

    assert_event_stream_head(&response);
    let written = chunks(response).await;
    assert_eq!(written, single(&expected));
    assert_only_protocol_blocks(&written);
    assert_eq!(node.subscribers(), 0);
}

#[tokio::test]
async fn the_depth_and_complexity_limits_are_answered_inside_the_stream() {
    let node = Node::ready();
    let too_deep = nested(MAX_DEPTH);
    let deep_enough = nested(MAX_DEPTH - 6);
    let too_complex = of_complexity(MAX_COMPLEXITY + 1);
    let complex_enough = of_complexity(MAX_COMPLEXITY);

    for (document, message) in [
        (&too_deep, Some("Query is nested too deep.")),
        (&deep_enough, None),
        (&too_complex, Some("Query is too complex.")),
        (&complex_enough, None),
    ] {
        let expected = node.json_transport(Request::new(document.as_str())).await;
        let written = chunks(node.answer(document).await).await;
        assert_eq!(written, single(&expected), "{message:?}");
        let result: Value = serde_json::from_str(&expected).unwrap();
        match message {
            Some(message) => assert_eq!(result["errors"][0]["message"], message),
            None => assert!(result.get("errors").is_none(), "{result}"),
        }
    }
    assert_eq!(node.subscribers(), 0);
}

#[tokio::test]
async fn a_document_error_keeps_its_message_and_locations_without_a_code() {
    let node = Node::ready();

    let written = chunks(node.answer("{ unknownField }").await).await;

    assert_eq!(
        single_result(&written),
        json!({"data": null, "errors": [{
            "message": r#"Unknown field "unknownField" on type "Query"."#,
            "locations": [{"line": 1, "column": 3}],
        }]})
    );
}

#[tokio::test]
async fn a_query_on_a_starting_node_is_answered_and_never_refused() {
    let node = Node::new(false, 4, Arc::new(StaticTopic::empty()));
    let document = "{ eventStreamStatus { state } }";

    let written = chunks(node.answer(document).await).await;

    assert_eq!(
        written,
        single(r#"{"data":{"eventStreamStatus":{"state":"starting"}}}"#)
    );
}

#[rstest]
#[case::two_aliases(r#"subscription { events(types: ["a"]) { a: data b: data } }"#)]
#[case::through_a_fragment(
    r#"subscription { events(types: ["a"]) { data ...F } } fragment F on Event { data }"#
)]
#[tokio::test]
async fn a_subscription_that_selects_the_event_data_twice_is_one_complexity_error(
    #[case] document: &str,
) {
    let node = Node::ready();
    let expected = node.json_transport(Request::new(document)).await;

    let written = chunks(node.answer(document).await).await;

    assert_eq!(written, single(&expected));
    assert_eq!(
        single_result(&written),
        json!({"data": null, "errors": [{"message": "Query is too complex."}]})
    );
    assert_eq!(node.subscribers(), 0);
    assert_eq!(node.state.status.snapshot().subscribers, 0);
}

/// The column of the field `events` in `document`, 1-based.
fn column(document: &str) -> usize {
    document.find("events").unwrap().saturating_add(1)
}

/// The `VALIDATION_FAILED` result of the field `events` of `document`.
fn validation_failed(document: &str, property_path: &str, code: &str, message: &str) -> Value {
    json!({"data": null, "errors": [{
        "message": "The request failed validation.",
        "locations": [{"line": 1, "column": column(document)}],
        "path": ["events"],
        "extensions": {
            "code": "VALIDATION_FAILED",
            "violations": [{"propertyPath": property_path, "message": message, "code": code}],
        },
    }]})
}

const TYPES_COUNT: &str = "types must hold 1 to 100 event type names";

/// One hundred and one distinct names.
fn too_many_names() -> String {
    let names: Vec<String> = (0..=100).map(|n| format!(r#""t{n}""#)).collect();
    format!(
        "subscription {{ events(types: [{}]) {{ id }} }}",
        names.join(",")
    )
}

#[rstest]
#[case::no_type(
    "subscription { events(types: []) { id } }".to_owned(), None,
    "types", "required", TYPES_COUNT
)]
#[case::an_empty_name(
    r#"subscription { events(types: ["a", ""]) { id } }"#.to_owned(), None,
    "types", "required", "types must not hold an empty event type name"
)]
#[case::a_name_that_is_too_long(
    format!(r#"subscription {{ events(types: ["{}"]) {{ id }} }}"#, "a".repeat(129)), None,
    "types", "length", "An event type must contain between 1 and 128 characters."
)]
#[case::a_name_that_breaks_the_pattern(
    r#"subscription { events(types: ["A"]) { id } }"#.to_owned(), None,
    "types", "pattern", "An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$."
)]
#[case::too_many_names(too_many_names(), None, "types", "max_items", TYPES_COUNT)]
#[case::a_malformed_header(
    r#"subscription { events(types: ["a"]) { id } }"#.to_owned(), Some("x"),
    "Last-Event-ID", "format", "Last-Event-ID must be a decimal unsigned 64-bit integer"
)]
#[case::a_malformed_argument_beside_a_valid_header(
    r#"subscription { events(types: ["a"], lastEventId: "-1") { id } }"#.to_owned(), Some("5"),
    "lastEventId", "format", "lastEventId must be a decimal unsigned 64-bit integer"
)]
#[case::types_before_the_cursor(
    r#"subscription { events(types: ["A"], lastEventId: "x") { id } }"#.to_owned(), Some("x"),
    "types", "pattern", "An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$."
)]
#[tokio::test]
async fn invalid_arguments_are_one_validation_error_inside_the_stream_and_register_nothing(
    #[case] document: String,
    #[case] header: Option<&str>,
    #[case] property_path: &str,
    #[case] code: &str,
    #[case] message: &str,
) {
    let node = Node::ready();
    let expected = validation_failed(&document, property_path, code, message);

    let response = node.answer_with(Request::new(document), header).await;

    assert_event_stream_head(&response);
    assert_eq!(single_result(&chunks(response).await), expected);
    assert_eq!(node.subscribers(), 0);
    assert_eq!(node.state.status.snapshot().subscribers, 0);
}

#[tokio::test]
async fn a_root_fragment_that_runs_nothing_is_the_comment_and_complete() {
    let node = Node::ready();

    let written = chunks(
        node.answer(
            r#"subscription { ...F } fragment F on Subscription { events(types: ["a"]) { id } }"#,
        )
        .await,
    )
    .await;

    assert_eq!(written, [":\n\n", "event: complete\ndata:\n\n"]);
    assert_eq!(node.subscribers(), 0);
}

// ---- refusals by the hub (FR-9 (c), FR-16) ----

/// Answers a valid subscription on `node`, which must refuse it with `detail`.
async fn assert_refused(node: &Node, detail: &str) {
    let held = node.subscribers();
    let (recorder, guard) = LogRecorder::install();

    let response = node.answer(EVENTS).await;
    drop(guard);

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()[CONTENT_TYPE], "application/problem+json");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"type": "about:blank", "title": "Service Unavailable", "status": 503, "detail": detail})
    );
    assert_eq!(node.subscribers(), held);
    let rejected = lines_of(&recorder, "subscription rejected");
    assert_eq!(rejected.len(), 1, "{:?}", recorder.lines());
    assert_eq!(rejected[0].level, Level::WARN);
    assert_eq!(rejected[0].field("detail"), Some(detail));
    assert_eq!(recorder.lines().len(), 1, "{:?}", recorder.lines());
}

#[tokio::test]
async fn a_starting_node_refuses_with_the_503_problem() {
    let node = Node::new(false, 4, Arc::new(StaticTopic::empty()));

    assert_refused(&node, "the node is starting").await;
}

#[tokio::test]
async fn a_node_that_is_shutting_down_refuses_with_the_503_problem() {
    let node = Node::ready();
    node.state.hub.close();

    assert_refused(&node, "the node is shutting down").await;
}

#[tokio::test(start_paused = true)]
async fn a_node_at_its_capacity_refuses_with_the_503_problem_and_keeps_its_subscribers() {
    let node = Node::new(true, 1, Arc::new(StaticTopic::empty()));
    let held = node.answer(EVENTS).await;
    assert_event_stream_head(&held);
    assert_eq!(node.subscribers(), 1);

    assert_refused(&node, "the node is at its subscriber capacity").await;

    drop(held);
    assert_eq!(node.subscribers(), 0);
}

#[tokio::test]
async fn shutting_down_comes_before_starting() {
    let node = Node::new(false, 4, Arc::new(StaticTopic::empty()));
    node.state.hub.close();

    assert_refused(&node, "the node is shutting down").await;
}

#[tokio::test(start_paused = true)]
async fn shutting_down_comes_before_the_capacity() {
    let node = Node::new(true, 1, Arc::new(StaticTopic::empty()));
    let held = node.answer(EVENTS).await;
    node.state.hub.close();

    assert_refused(&node, "the node is shutting down").await;

    drop(held);
}

// ---- the request budget and the end of a stream (FR-9, FR-19) ----

#[tokio::test(start_paused = true)]
async fn dropping_respond_after_the_registration_and_before_the_head_frees_the_slot() {
    let node = Node::ready();
    let mut answer = Box::pin(respond(&node.schema, Request::new(EVENTS), None));

    assert!(still_pending(answer.as_mut()).await);
    assert_eq!(node.subscribers(), 1);

    drop(answer);
    assert_eq!(node.subscribers(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_closed_hub_ends_the_stream_after_its_queued_results_without_complete() {
    let node = Node::ready();
    let mut body = node.answer(EVENTS).await.into_body();
    assert_eq!(chunk(&mut body).await.as_deref(), Some(":\n\n"));
    node.pass(1_001, "a");
    node.pass(1_002, "a");

    node.state.hub.close();

    assert_eq!(chunk(&mut body).await, Some(event_block(1_001)));
    assert_eq!(chunk(&mut body).await, Some(event_block(1_002)));
    assert_eq!(chunk(&mut body).await, None);
}

#[tokio::test(start_paused = true)]
async fn a_client_that_drops_the_body_frees_its_slot_at_once() {
    let node = Node::ready();
    let mut body = node.answer(EVENTS).await.into_body();
    node.pass(1_001, "a");
    assert_eq!(chunk(&mut body).await.as_deref(), Some(":\n\n"));
    assert_eq!(chunk(&mut body).await, Some(event_block(1_001)));
    assert_eq!(node.subscribers(), 1);

    drop(body);

    assert_eq!(node.subscribers(), 0);
    assert_eq!(node.state.status.snapshot().subscribers, 0);
}

// ---- logs (FR-22, NFR-9) ----

#[tokio::test(start_paused = true)]
async fn no_entry_is_logged_for_a_query_an_error_an_invalid_argument_or_a_whole_subscription() {
    let node = Node::ready();
    let (recorder, guard) = LogRecorder::install();

    chunks(node.answer("{ eventTypes { name } }").await).await;
    chunks(node.answer("{ unknownField }").await).await;
    chunks(
        node.answer(r#"subscription { events(types: ["A"]) { id } }"#)
            .await,
    )
    .await;
    let mut body = node.answer(EVENTS).await.into_body();
    node.pass(1_001, "a");
    assert_eq!(chunk(&mut body).await.as_deref(), Some(":\n\n"));
    assert_eq!(chunk(&mut body).await, Some(event_block(1_001)));
    drop(body);
    drop(guard);

    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
    assert_eq!(node.subscribers(), 0);
}
