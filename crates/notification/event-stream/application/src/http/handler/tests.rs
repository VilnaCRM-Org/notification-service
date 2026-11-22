use std::any::TypeId;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::future::poll_fn;
use std::pin::pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::header::{
    CACHE_CONTROL, CONNECTION, CONTENT_TYPE, HeaderName, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION,
    UPGRADE,
};
use axum::http::response::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode};
use event_stream_domain::{
    Cursor, EventType, Offset, StoredEvent, TopicError, TopicRecord, TopicReplay,
};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde_json::{Value, json};
use shared_application::http::Problem;
use shared_domain::{ErasedBusError, ErasedMessage, QueryBus, QueryHandler as _};
use test_support::bus::FakeQueryBus;
use test_support::fixture::stored_event;
use test_support::log::{LogLine, LogRecorder};
use test_support::static_topic::StaticTopic;
use test_support::topic::ScriptedTopicReplay;
use tokio::sync::watch;
use tokio::time::timeout;
use tower::ServiceExt as _;
use tracing::Level;

use super::MAX_CHUNK_BYTES;
use crate::catalog::EventCatalog;
use crate::http::dto::{
    Command, EXAMPLE_CURSOR, EXAMPLE_TYPE, TypesError, parse_cursor, parse_types, read_cursor,
    types_problem,
};
use crate::http::frame::{Frame, error_json, ws_json};
use crate::http::hub::{Hub, HubError, HubSettings};
use crate::http::session::Session;
use crate::http::subscription::{ReplaySettings, Subscription, Subscriptions};
use crate::http::{EventStreamState, StreamSettings, router};
use crate::query::list_event_types::{ListEventTypes, ListEventTypesHandler};
use crate::status::StatusSource;
use crate::tail::{EventPublisher as _, TailState, TopicFault};
use crate::window::Window;

mod json_schema;

const PROBLEM: &str = "application/problem+json";

/// Asks the real handler, as the in-memory bus does.
#[derive(Debug)]
struct HandlerQueryBus(ListEventTypesHandler);

#[async_trait::async_trait]
impl QueryBus for HandlerQueryBus {
    async fn ask_erased(
        &self,
        _query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError> {
        let query = query
            .downcast::<ListEventTypes>()
            .map_err(|_other| ErasedBusError::NotRegistered { message: name })?;
        match self.0.handle(*query).await {
            Ok(output) => Ok(Box::new(output)),
            Err(error) => Err(ErasedBusError::Handler(Box::new(error))),
        }
    }
}

const SETTINGS: HubSettings = HubSettings {
    max_subscribers: 4,
    buffer: 2,
};

/// The keep-alive interval of these tests; not the default of 15 s, so a lost setting shows.
const KEEP_ALIVE: Duration = Duration::from_secs(7);

/// One millisecond less than [`KEEP_ALIVE`].
const JUST_BEFORE_KEEP_ALIVE: Duration = Duration::from_millis(6_999);

/// The bound of the awaited head of a response, and the quiet time after the last frame of a
/// stream; shorter than [`KEEP_ALIVE`], so a missing frame is not hidden by a keep-alive comment.
const WAIT: Duration = Duration::from_secs(5);

/// The last millisecond of [`KEEP_ALIVE`].
const ONE_MILLISECOND: Duration = Duration::from_millis(1);

/// The polls a read of a body allows, each followed by a yield to the scheduler. Under paused time
/// a task that keeps yielding stops the clock, so a `timeout` around a read of a body that only
/// yields could never fire; a bounded count of polls fails an assertion instead of hanging the
/// test, and the tests move the clock themselves.
const POLLS: usize = 64;

const REPLAY: ReplaySettings = ReplaySettings {
    max_events: 1_000_000,
    max_data_bytes: 65_536,
    permits: 4,
};

/// A state whose subscriptions and status read the tail state of `tail`, `window`, `hub` and
/// `replay`.
fn state_on(
    query_bus: Arc<dyn QueryBus>,
    tail: &watch::Receiver<TailState>,
    window: Arc<Window>,
    hub: Arc<Hub>,
    replay: Arc<dyn TopicReplay>,
) -> EventStreamState {
    let subscriptions = Subscriptions::new(
        Arc::<Hub>::clone(&hub),
        Arc::<Window>::clone(&window),
        replay,
        tail.clone(),
        REPLAY,
    );
    EventStreamState {
        query_bus,
        subscriptions: Arc::new(subscriptions),
        status: Arc::new(StatusSource::new(
            tail.clone(),
            window,
            Arc::<Hub>::clone(&hub),
        )),
        hub,
        settings: StreamSettings {
            keep_alive: KEEP_ALIVE,
        },
    }
}

/// A state whose status reads `tail`, `window` and `hub`; its replay is an empty topic.
fn state_with(
    query_bus: Arc<dyn QueryBus>,
    tail: TailState,
    window: Arc<Window>,
    hub: Arc<Hub>,
) -> EventStreamState {
    let (_sender, receiver) = watch::channel(tail);
    state_on(
        query_bus,
        &receiver,
        window,
        hub,
        Arc::new(StaticTopic::empty()),
    )
}

/// A state of a node that has not started, over `query_bus`.
fn starting(query_bus: Arc<dyn QueryBus>) -> EventStreamState {
    state_with(
        query_bus,
        TailState::default(),
        Arc::new(Window::new(10, usize::MAX)),
        Hub::new(SETTINGS),
    )
}

fn state(names: &[&str]) -> EventStreamState {
    let types = names
        .iter()
        .map(|name| EventType::new((*name).to_owned()).unwrap())
        .collect::<BTreeSet<_>>();
    starting(Arc::new(HandlerQueryBus(ListEventTypesHandler::new(
        Arc::new(EventCatalog::new(types)),
    ))))
}

fn three() -> EventStreamState {
    state(&["user.deleted", "order.paid", "user.created"])
}

fn app(state: EventStreamState) -> Router {
    let (routes, _document) = router::<EventStreamState>().split_for_parts();
    routes.with_state(state)
}

struct Reply {
    status: StatusCode,
    content_type: String,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

async fn send_request(state: EventStreamState, request: Request<Body>) -> Reply {
    timeout(WAIT, async {
        let response = app(state).oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let content_type = parts
            .headers
            .get(CONTENT_TYPE)
            .map(|value| value.to_str().unwrap().to_owned())
            .unwrap_or_default();
        Reply {
            status: parts.status,
            content_type,
            body: body.collect().await.unwrap().to_bytes().to_vec(),
        }
    })
    .await
    .expect("the request answers in time")
}

async fn send(state: EventStreamState, uri: &str) -> Reply {
    send_request(
        state,
        Request::builder().uri(uri).body(Body::empty()).unwrap(),
    )
    .await
}

#[tokio::test]
async fn an_empty_catalog_answers_an_empty_array() {
    let reply = send(state(&[]), "/event-types").await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.content_type, "application/json");
    assert_eq!(reply.body, b"[]");
}

#[tokio::test]
async fn the_catalog_answers_each_name_once_in_byte_order() {
    let reply = send(
        state(&[
            "user.created",
            "order.paid",
            "order-paid",
            "order_paid",
            "order.paid",
        ]),
        "/event-types",
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.json(),
        json!([
            {"name": "order-paid"},
            {"name": "order.paid"},
            {"name": "order_paid"},
            {"name": "user.created"}
        ])
    );
}

#[rstest]
#[case::second_of_one("itemsPerPage=1&page=2", json!([{"name": "user.created"}]))]
#[case::beyond_the_last("page=4&itemsPerPage=1", json!([]))]
#[case::smallest_size("itemsPerPage=1", json!([{"name": "order.paid"}]))]
#[case::largest_size(
    "itemsPerPage=100",
    json!([{"name": "order.paid"}, {"name": "user.created"}, {"name": "user.deleted"}])
)]
#[case::unknown_parameters_are_ignored(
    "color=red",
    json!([{"name": "order.paid"}, {"name": "user.created"}, {"name": "user.deleted"}])
)]
#[tokio::test]
async fn the_page_parameters_select_the_page(#[case] query: &str, #[case] expected: Value) {
    let reply = send(three(), &format!("/event-types?{query}")).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), expected);
}

#[rstest]
#[case::page_zero("page=0", "page", "page must be 1 or greater")]
#[case::page_negative("page=-1", "page", "page must be 1 or greater")]
#[case::no_items(
    "itemsPerPage=0",
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
#[case::negative_items(
    "itemsPerPage=-3",
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
#[case::too_many_items(
    "itemsPerPage=101",
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
#[case::both_reports_the_page("page=0&itemsPerPage=0", "page", "page must be 1 or greater")]
#[tokio::test]
async fn a_page_outside_its_bounds_is_422_with_one_range_violation(
    #[case] query: &str,
    #[case] property: &str,
    #[case] message: &str,
) {
    let reply = send(three(), &format!("/event-types?{query}")).await;

    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.content_type, PROBLEM);
    let body = reply.json();
    assert_eq!(body["type"], "/problems/validation-failed");
    assert_eq!(body["status"], 422);
    assert_eq!(
        body["violations"],
        json!([{"propertyPath": property, "message": message, "code": "range"}])
    );
}

#[rstest]
#[case::text("page=abc", "page: invalid digit found in string")]
#[case::fraction("itemsPerPage=1.5", "itemsPerPage: invalid digit found in string")]
#[case::beyond_32_bits("page=4294967296", "page: number too large to fit in target type")]
#[tokio::test]
async fn a_value_that_is_not_a_32_bit_integer_is_400_with_the_parser_text(
    #[case] query: &str,
    #[case] parser_text: &str,
) {
    let queries = Arc::new(FakeQueryBus::unregistered());
    let state = starting(Arc::<FakeQueryBus>::clone(&queries));

    let reply = send(state, &format!("/event-types?{query}")).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.content_type, PROBLEM);
    let body = reply.json();
    assert_eq!(body["type"], "about:blank");
    assert_eq!(
        body["detail"],
        format!("Failed to deserialize query string: {parser_text}")
    );
    assert_eq!(queries.asked(), Vec::<&str>::new());
}

#[tokio::test]
async fn the_handler_asks_the_list_query_once() {
    let queries = Arc::new(FakeQueryBus::answering(|query: ListEventTypes| {
        assert_eq!((query.page, query.items_per_page), (3, 7));
        Ok(Vec::new())
    }));
    let state = starting(Arc::<FakeQueryBus>::clone(&queries));

    let reply = send(state, "/event-types?page=3&itemsPerPage=7").await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(queries.asked(), vec!["ListEventTypes"]);
}

#[tokio::test]
async fn the_router_serves_only_the_registered_method_and_path() {
    let wrong_method = send_request(
        three(),
        Request::builder()
            .method(Method::POST)
            .uri("/event-types")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let unknown = send(three(), "/event-types/a").await;

    assert_eq!(wrong_method.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

// ---- status ----

const STATUS_FIELDS: [&str; 12] = [
    "brokerReachable",
    "endOffset",
    "lag",
    "position",
    "reason",
    "rejectedRecords",
    "replayCapped",
    "retentionLosses",
    "state",
    "subscribers",
    "windowFirst",
    "windowLast",
];

fn idle_bus() -> Arc<dyn QueryBus> {
    Arc::new(FakeQueryBus::unregistered())
}

fn field_names(body: &Value) -> Vec<String> {
    let mut names: Vec<String> = body.as_object().unwrap().keys().cloned().collect();
    names.sort_unstable();
    names
}

#[tokio::test]
async fn a_starting_node_answers_200_with_the_nulls_of_an_unknown_position() {
    let queries = Arc::new(FakeQueryBus::unregistered());

    let reply = send(
        starting(Arc::<FakeQueryBus>::clone(&queries)),
        "/events/status",
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.content_type, "application/json");
    assert_eq!(
        reply.json(),
        json!({
            "state": "starting",
            "reason": "catching-up",
            "brokerReachable": false,
            "position": null,
            "endOffset": null,
            "lag": null,
            "windowFirst": null,
            "windowLast": null,
            "subscribers": 0,
            "rejectedRecords": "0",
            "retentionLosses": "0",
            "replayCapped": "0"
        })
    );
    assert_eq!(queries.asked(), Vec::<&str>::new());
}

#[tokio::test]
async fn a_ready_node_answers_its_position_window_and_subscribers() {
    let window = Arc::new(Window::new(10, usize::MAX));
    window.restart(Cursor::new(30));
    window.push(Arc::new(stored_event(35, "user.created", "{}")));
    let hub = Hub::new(SETTINGS);
    hub.start(Cursor::new(35));
    let types = BTreeSet::from([EventType::new("user.created".to_owned()).unwrap()]);
    let _first = hub.register(&types).unwrap();
    let _second = hub.register(&types).unwrap();
    let tail = TailState {
        started: true,
        broker_reachable: true,
        position: Some(Cursor::new(u64::MAX)),
        end: Some(Cursor::new(u64::MAX)),
        rejected: 3,
        retention_losses: 2,
        ..TailState::default()
    };

    let reply = send(state_with(idle_bus(), tail, window, hub), "/events/status").await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.json(),
        json!({
            "state": "ready",
            "reason": null,
            "brokerReachable": true,
            "position": "18446744073709551615",
            "endOffset": "18446744073709551615",
            "lag": "0",
            "windowFirst": "30",
            "windowLast": "35",
            "subscribers": 2,
            "rejectedRecords": "3",
            "retentionLosses": "2",
            "replayCapped": "0"
        })
    );
}

#[rstest]
#[case::unreachable(Some(TopicFault::Unreachable), "broker-unreachable")]
#[case::missing(Some(TopicFault::Missing), "topic-not-found")]
#[case::partitions(Some(TopicFault::Partitions(2)), "partition-count")]
#[case::catching_up(None, "catching-up")]
#[tokio::test]
async fn a_starting_node_names_its_reason(#[case] fault: Option<TopicFault>, #[case] reason: &str) {
    let tail = TailState {
        fault,
        ..TailState::default()
    };
    let state = state_with(
        idle_bus(),
        tail,
        Arc::new(Window::new(10, usize::MAX)),
        Hub::new(SETTINGS),
    );

    let reply = send(state, "/events/status").await;

    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.json();
    assert_eq!(body["state"], "starting");
    assert_eq!(body["reason"], reason);
    assert_eq!(field_names(&body), STATUS_FIELDS);
}

#[tokio::test]
async fn a_closed_hub_answers_200_shutting_down_and_not_a_503() {
    let hub = Hub::new(SETTINGS);
    hub.close();
    let tail = TailState {
        started: true,
        ..TailState::default()
    };
    let state = state_with(idle_bus(), tail, Arc::new(Window::new(10, usize::MAX)), hub);

    let reply = send(state, "/events/status").await;

    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.json();
    assert_eq!(body["state"], "shutting-down");
    assert_eq!(body["reason"], Value::Null);
    assert_eq!(field_names(&body), STATUS_FIELDS);
}

#[tokio::test]
async fn the_status_reads_the_tail_state_at_each_request() {
    let (sender, receiver) = watch::channel(TailState::default());
    let state = state_on(
        idle_bus(),
        &receiver,
        Arc::new(Window::new(10, usize::MAX)),
        Hub::new(SETTINGS),
        Arc::new(StaticTopic::empty()),
    );

    let before = send(state.clone(), "/events/status").await;
    sender.send_replace(TailState {
        started: true,
        ..TailState::default()
    });
    let after = send(state, "/events/status").await;

    assert_eq!(before.json()["state"], "starting");
    assert_eq!(after.json()["state"], "ready");
}

#[tokio::test]
async fn the_status_route_serves_only_get() {
    let reply = send_request(
        three(),
        Request::builder()
            .method(Method::POST)
            .uri("/events/status")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
}

// ---- event stream ----

const HUB: HubSettings = HubSettings {
    max_subscribers: 8,
    buffer: 64,
};

/// The event object of the event `id`, as the envelope parser writes it.
fn event_json(id: u64, event_type: &str) -> String {
    format!(
        r#"{{"id":"00000000-0000-0000-0000-{id:012x}","type":"{event_type}","occurredAt":"2026-11-14T10:00:00Z","data":{{"n":{id}}}}}"#
    )
}

fn event(id: u64, event_type: &str) -> StoredEvent {
    stored_event(id, event_type, &event_json(id, event_type))
}

/// The SSE frames of the events, joined, as one chunk writes them.
fn frames(events: &[(u64, &str)]) -> String {
    events
        .iter()
        .map(|(id, event_type)| frame(*id, event_type))
        .collect()
}

/// The SSE frame of the event `id`.
fn frame(id: u64, event_type: &str) -> String {
    format!(
        "id: {id}\nevent: {event_type}\ndata: {}\n\n",
        event_json(id, event_type)
    )
}

/// A node whose hub was started at `position` and whose window floor is `position`.
struct Node {
    hub: Arc<Hub>,
    window: Arc<Window>,
    _tail: watch::Sender<TailState>,
    state: EventStreamState,
}

fn node_on(position: u64, replay: Arc<dyn TopicReplay>, settings: HubSettings) -> Node {
    let hub = Hub::new(settings);
    hub.start(Cursor::new(position));
    let window = Arc::new(Window::new(100, usize::MAX));
    window.restart(Cursor::new(position));
    let (tail, receiver) = watch::channel(TailState {
        started: true,
        ..TailState::default()
    });
    let state = state_on(
        idle_bus(),
        &receiver,
        Arc::<Window>::clone(&window),
        Arc::<Hub>::clone(&hub),
        replay,
    );
    Node {
        hub,
        window,
        _tail: tail,
        state,
    }
}

/// A ready node at `position` whose replay is an empty topic.
fn ready(position: u64) -> Node {
    node_on(position, Arc::new(StaticTopic::empty()), HUB)
}

impl Node {
    /// What the tail does with a valid event: the window first, then the hub.
    fn pass(&self, event: &StoredEvent) {
        self.window.push(Arc::new(event.clone()));
        self.hub.publish(event);
    }
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn get_after(uri: &str, last_event_id: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("last-event-id", last_event_id)
        .body(Body::empty())
        .unwrap()
}

/// Sends the request and returns the head and the body, which may stay open.
async fn open(state: EventStreamState, request: Request<Body>) -> (Parts, Body) {
    timeout(WAIT, app(state).oneshot(request))
        .await
        .expect("the head answers in time")
        .unwrap()
        .into_parts()
}

/// Polls `future` with the context of the test task at most [`POLLS`] times, with a yield to the
/// scheduler after each poll that pended; `None` when it is still pending after the last one.
async fn within_polls<F: Future>(future: F) -> Option<F::Output> {
    let mut future = pin!(future);
    for _ in 0..POLLS {
        if let Poll::Ready(output) =
            poll_fn(|context| Poll::Ready(future.as_mut().poll(context))).await
        {
            return Some(output);
        }
        tokio::task::yield_now().await;
    }
    None
}

/// The next chunk of the body as text, `None` when the body ended; panics when the body is still
/// pending after [`POLLS`] polls. The clock does not move.
async fn chunk(body: &mut Body) -> Option<String> {
    let frame = within_polls(body.frame())
        .await
        .expect("a chunk within the polls")?;
    let data = frame.unwrap().into_data().unwrap();
    Some(String::from_utf8(data.to_vec()).unwrap())
}

/// True when the body writes nothing and stays open while the clock moves on by `quiet`: polled
/// before and after the clock moved, it is still pending after [`POLLS`] polls.
async fn silent(body: &mut Body, quiet: Duration) -> bool {
    let mut frame = pin!(body.frame());
    let before = poll_fn(|context| Poll::Ready(frame.as_mut().poll(context))).await;
    tokio::time::advance(quiet).await;
    before.is_pending() && within_polls(frame).await.is_none()
}

fn header_text(head: &Parts, name: &str) -> String {
    head.headers[name].to_str().unwrap().to_owned()
}

fn lines_of(recorder: &LogRecorder, message: &str) -> Vec<LogLine> {
    recorder
        .lines()
        .into_iter()
        .filter(|line| line.message == message)
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_ready_node_opens_an_event_stream_with_the_head_of_fr_5_and_keeps_it_open() {
    let node = ready(1_000);

    let (head, mut body) = open(node.state.clone(), get("/events/stream?types=user.created")).await;

    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(
        header_text(&head, CONTENT_TYPE.as_str()),
        "text/event-stream"
    );
    assert_eq!(header_text(&head, CACHE_CONTROL.as_str()), "no-cache");
    assert_eq!(header_text(&head, "x-accel-buffering"), "no");
    assert_eq!(node.hub.subscriber_count(), 1);
    assert!(silent(&mut body, JUST_BEFORE_KEEP_ALIVE).await);
    assert_eq!(node.hub.subscriber_count(), 1);
    drop(body);
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn only_the_subscribed_type_arrives_as_one_frame_per_event_in_id_order() {
    let node = ready(1_000);
    let (_head, mut body) =
        open(node.state.clone(), get("/events/stream?types=user.created")).await;

    node.pass(&event(1_001, "user.created"));
    node.pass(&event(1_002, "order.paid"));
    node.pass(&event(1_003, "user.created"));

    assert_eq!(
        chunk(&mut body).await,
        Some(frames(&[(1_001, "user.created"), (1_003, "user.created")]))
    );
    assert!(silent(&mut body, WAIT).await);
}

#[tokio::test(start_paused = true)]
async fn every_listed_type_arrives_and_a_repeated_name_counts_once() {
    let node = ready(1_000);
    let (head, mut body) = open(
        node.state.clone(),
        get("/events/stream?types=order.paid,user.created,order.paid"),
    )
    .await;

    node.pass(&event(1_001, "user.created"));
    node.pass(&event(1_002, "order.paid"));
    node.pass(&event(1_003, "user.deleted"));
    node.pass(&event(1_004, "user.created"));

    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(
        chunk(&mut body).await,
        Some(frames(&[
            (1_001, "user.created"),
            (1_002, "order.paid"),
            (1_004, "user.created")
        ]))
    );
    assert!(silent(&mut body, WAIT).await);
}

/// A record whose envelope is written over several lines and whose data holds a line break inside
/// a string.
fn multi_line_record() -> TopicRecord {
    let value = "{\n  \"id\": \"8F14E45F-CEEA-467F-A8F4-3D1B5C4E1A2B\",\n  \"type\": \"user.created\",\n  \
                 \"occurredAt\": \"2026-11-14T10:00:00Z\",\n  \"data\": {\"text\": \"line one\\nline two\"}\n}";
    TopicRecord {
        offset: Offset::new(0),
        value: Some(value.as_bytes().to_vec()),
    }
}

#[tokio::test(start_paused = true)]
async fn an_event_whose_data_holds_a_line_break_is_one_data_line_the_client_parses_unchanged() {
    let topic = StaticTopic::new(vec![multi_line_record()], Cursor::START, Cursor::new(1));
    let node = node_on(1, Arc::new(topic), HUB);

    let (_head, mut body) = open(
        node.state.clone(),
        get("/events/stream?types=user.created&lastEventId=0"),
    )
    .await;

    let text = chunk(&mut body).await.unwrap();
    let lines: Vec<&str> = text.split('\n').collect();
    assert_eq!(lines.len(), 5, "{text:?}");
    assert_eq!(lines[0], "id: 1");
    assert_eq!(lines[1], "event: user.created");
    assert_eq!(lines[3..], ["", ""]);
    let data = lines[2].strip_prefix("data: ").unwrap();
    assert_eq!(
        data,
        r#"{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"text":"line one\nline two"}}"#
    );
    let parsed: Value = serde_json::from_str(data).unwrap();
    assert_eq!(parsed["data"]["text"], "line one\nline two");
}

#[tokio::test(start_paused = true)]
async fn without_a_frame_for_the_keep_alive_interval_the_stream_writes_the_comment_line() {
    let node = ready(1_000);
    let (_head, mut body) =
        open(node.state.clone(), get("/events/stream?types=user.created")).await;

    assert!(silent(&mut body, JUST_BEFORE_KEEP_ALIVE).await);
    tokio::time::advance(ONE_MILLISECOND).await;
    assert_eq!(chunk(&mut body).await, Some(":\n\n".to_owned()));
    assert!(silent(&mut body, Duration::from_secs(3)).await);

    node.pass(&event(1_001, "user.created"));
    assert_eq!(chunk(&mut body).await, Some(frame(1_001, "user.created")));
    assert!(silent(&mut body, JUST_BEFORE_KEEP_ALIVE).await);
    tokio::time::advance(ONE_MILLISECOND).await;
    assert_eq!(chunk(&mut body).await, Some(":\n\n".to_owned()));
}

#[tokio::test(start_paused = true)]
async fn the_header_cursor_wins_over_the_parameter_and_resumes_from_the_window() {
    let node = ready(0);
    for id in 1..=10 {
        node.pass(&event(id, "user.created"));
    }

    let (head, mut body) = open(
        node.state.clone(),
        get_after("/events/stream?types=user.created&lastEventId=5", "8"),
    )
    .await;

    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(
        chunk(&mut body).await,
        Some(frames(&[(9, "user.created"), (10, "user.created")]))
    );
    assert!(silent(&mut body, WAIT).await);
}

#[tokio::test(start_paused = true)]
async fn the_parameter_cursor_resumes_when_no_header_is_sent() {
    let node = ready(0);
    for id in 1..=4 {
        node.pass(&event(id, "user.created"));
    }

    let (_head, mut body) = open(
        node.state.clone(),
        get("/events/stream?types=user.created&lastEventId=2"),
    )
    .await;

    assert_eq!(
        chunk(&mut body).await,
        Some(frames(&[(3, "user.created"), (4, "user.created")]))
    );
    assert!(silent(&mut body, WAIT).await);
}

fn unavailable() -> Result<event_stream_domain::Batch, TopicError> {
    Err(TopicError::Unavailable("the broker is away".into()))
}

#[tokio::test(start_paused = true)]
async fn a_replay_that_cannot_read_kafka_ends_the_response_and_logs_replay_failed() {
    let (recorder, _guard) = LogRecorder::install();
    let replay = ScriptedTopicReplay::new();
    replay.push_page(unavailable());
    let node = node_on(900, Arc::new(replay.clone()), HUB);

    let (head, mut body) = open(
        node.state.clone(),
        get_after("/events/stream?types=user.created", "100"),
    )
    .await;

    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(chunk(&mut body).await, None);
    assert_eq!(replay.calls(), [Cursor::new(100)]);
    assert_eq!(
        lines_of(&recorder, "replay failed"),
        [LogLine {
            level: Level::WARN,
            target: "event_stream_application::http::subscription".to_owned(),
            message: "replay failed".to_owned(),
            fields: vec![
                ("transport".to_owned(), "sse".to_owned()),
                (
                    "error".to_owned(),
                    "the topic is unavailable: the broker is away".to_owned()
                ),
            ],
        }]
    );
}

#[tokio::test(start_paused = true)]
async fn at_shutdown_the_response_ends_after_the_queued_frames_without_a_log_entry() {
    let (recorder, _guard) = LogRecorder::install();
    let node = ready(1_000);
    let (_head, mut body) =
        open(node.state.clone(), get("/events/stream?types=user.created")).await;
    node.pass(&event(1_001, "user.created"));
    node.pass(&event(1_002, "user.created"));

    node.hub.close();

    assert_eq!(
        chunk(&mut body).await,
        Some(frames(&[(1_001, "user.created"), (1_002, "user.created")]))
    );
    assert_eq!(chunk(&mut body).await, None);
    assert_eq!(lines_of(&recorder, "replay failed"), Vec::<LogLine>::new());
}

/// The event `id` (of four digits) whose SSE block takes exactly `block_bytes`.
fn sized_event(id: u64, block_bytes: usize) -> StoredEvent {
    let wrapper = "id: 1001\nevent: user.created\ndata: {\"pad\":\"\"}\n\n".len();
    let json = format!(
        r#"{{"pad":"{}"}}"#,
        "x".repeat(block_bytes.saturating_sub(wrapper))
    );
    stored_event(id, "user.created", &json)
}

/// The event `id` whose SSE block takes exactly 8 KiB.
fn eighth_chunk_event(id: u64) -> StoredEvent {
    sized_event(id, MAX_CHUNK_BYTES / 4)
}

#[tokio::test(start_paused = true)]
async fn the_ready_frames_are_joined_into_chunks_up_to_32_kib() {
    let node = ready(1_000);
    let (_head, mut body) =
        open(node.state.clone(), get("/events/stream?types=user.created")).await;
    for id in 1_001..=1_006 {
        node.pass(&eighth_chunk_event(id));
    }

    let first = chunk(&mut body).await.unwrap();
    let second = chunk(&mut body).await.unwrap();

    assert_eq!(MAX_CHUNK_BYTES, 32_768);
    assert_eq!(first.len(), MAX_CHUNK_BYTES);
    assert_eq!(second.len(), MAX_CHUNK_BYTES / 2);
    let blocks: String = (1_001..=1_006)
        .map(|id| String::from_utf8(Frame::encode(&eighth_chunk_event(id)).sse().to_vec()).unwrap())
        .collect();
    assert_eq!(format!("{first}{second}"), blocks);
    assert!(silent(&mut body, WAIT).await);
}

#[tokio::test(start_paused = true)]
async fn a_frame_of_32_kib_or_more_is_a_chunk_of_its_own() {
    let node = ready(1_000);
    let (_head, mut body) =
        open(node.state.clone(), get("/events/stream?types=user.created")).await;
    let big = format!(r#"{{"pad":"{}"}}"#, "x".repeat(MAX_CHUNK_BYTES));
    node.pass(&stored_event(1_001, "user.created", &big));
    node.pass(&event(1_002, "user.created"));

    let first = chunk(&mut body).await.unwrap();

    assert!(first.ends_with(&format!("data: {big}\n\n")), "{first}");
    assert!(first.starts_with("id: 1001\n"), "{first}");
    assert_eq!(chunk(&mut body).await, Some(frame(1_002, "user.created")));
}

#[tokio::test(start_paused = true)]
async fn a_frame_of_exactly_32_kib_is_a_chunk_of_its_own() {
    let node = ready(1_000);
    let (_head, mut body) =
        open(node.state.clone(), get("/events/stream?types=user.created")).await;
    let exact = sized_event(1_001, MAX_CHUNK_BYTES);
    node.pass(&exact);
    node.pass(&event(1_002, "user.created"));

    let first = chunk(&mut body).await.unwrap();

    assert_eq!(first.len(), MAX_CHUNK_BYTES);
    assert_eq!(
        first,
        String::from_utf8(Frame::encode(&exact).sse().to_vec()).unwrap()
    );
    assert_eq!(chunk(&mut body).await, Some(frame(1_002, "user.created")));
}

#[tokio::test(start_paused = true)]
async fn a_frame_just_below_32_kib_is_joined_by_the_next_ready_frame() {
    let node = ready(1_000);
    let (_head, mut body) =
        open(node.state.clone(), get("/events/stream?types=user.created")).await;
    let below = sized_event(1_001, MAX_CHUNK_BYTES.saturating_sub(1));
    node.pass(&below);
    node.pass(&event(1_002, "user.created"));

    let first = chunk(&mut body).await.unwrap();

    assert_eq!(
        first,
        format!(
            "{}{}",
            String::from_utf8(Frame::encode(&below).sse().to_vec()).unwrap(),
            frame(1_002, "user.created")
        )
    );
    assert!(silent(&mut body, WAIT).await);
}

#[rstest]
#[case::missing("types", "")]
#[case::missing_with_cursor("types", "lastEventId=3")]
#[case::duplicate("types", "types=a&types=b")]
#[tokio::test(start_paused = true)]
async fn a_types_parameter_that_cannot_be_read_is_400_naming_it(
    #[case] named: &str,
    #[case] query: &str,
) {
    let node = ready(1_000);

    let reply = send(node.state.clone(), &format!("/events/stream?{query}")).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.content_type, PROBLEM);
    let body = reply.json();
    assert_eq!(body["type"], "about:blank");
    let detail = body["detail"].as_str().unwrap();
    assert!(
        detail.starts_with("Failed to deserialize query string: "),
        "{detail}"
    );
    assert!(detail.contains(&format!("`{named}`")), "{detail}");
    assert_eq!(node.hub.subscriber_count(), 0);
}

/// `count` distinct valid names joined with commas.
fn distinct_names(count: usize) -> String {
    (0..count)
        .map(|index| format!("t{index}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[rstest]
#[case::empty("types=", "required")]
#[case::empty_entry("types=a,,b", "required")]
#[case::pattern("types=User.Created", "pattern")]
#[case::length(&format!("types={}", "a".repeat(129)), "length")]
#[case::max_items(&format!("types={}", distinct_names(101)), "max_items")]
#[case::before_a_bad_cursor("types=&lastEventId=abc", "required")]
#[tokio::test(start_paused = true)]
async fn invalid_types_are_422_with_one_violation_on_types(
    #[case] query: &str,
    #[case] code: &str,
) {
    let node = ready(1_000);

    let reply = send(node.state.clone(), &format!("/events/stream?{query}")).await;

    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.content_type, PROBLEM);
    let body = reply.json();
    assert_eq!(body["type"], "/problems/validation-failed");
    let violations = body["violations"].as_array().unwrap();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["propertyPath"], "types");
    assert_eq!(violations[0]["code"], code);
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_hundred_distinct_names_with_repeats_open_a_stream() {
    let node = ready(1_000);
    let names = format!("{0},{0}", distinct_names(100));

    let (head, _body) = open(
        node.state.clone(),
        get(&format!("/events/stream?types={names}")),
    )
    .await;

    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(node.hub.type_count(), 100);
}

#[rstest]
#[case::empty("")]
#[case::text("abc")]
#[case::negative("-1")]
#[case::overflow("18446744073709551616")]
#[tokio::test(start_paused = true)]
async fn an_invalid_cursor_is_400_naming_its_carrier(#[case] value: &str) {
    let node = ready(1_000);

    let by_header = send_request(
        node.state.clone(),
        get_after("/events/stream?types=a&lastEventId=5", value),
    )
    .await;
    let by_parameter = send(
        node.state.clone(),
        &format!("/events/stream?types=a&lastEventId={value}"),
    )
    .await;
    let beside_a_valid_header = send_request(
        node.state.clone(),
        get_after(&format!("/events/stream?types=a&lastEventId={value}"), "5"),
    )
    .await;

    for (reply, carrier) in [
        (by_header, "Last-Event-ID"),
        (by_parameter, "lastEventId"),
        (beside_a_valid_header, "lastEventId"),
    ] {
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{carrier}");
        assert_eq!(reply.content_type, PROBLEM);
        assert_eq!(
            reply.json()["detail"],
            format!("{carrier} must be a decimal unsigned 64-bit integer")
        );
    }
    assert_eq!(node.hub.subscriber_count(), 0);
}

/// The request of the Schemathesis check ST201 (schema-violating request accepted): a valid
/// `Last-Event-ID` beside an empty `lastEventId`, which the parameter schema refuses. It is
/// refused before the node is asked, so a starting node answers it the same.
#[tokio::test(start_paused = true)]
async fn a_valid_header_beside_an_empty_parameter_is_the_400_problem_naming_the_parameter() {
    for node in [ready(1_000), refusing(8, Life::Unstarted)] {
        let request = get_after("/events/stream?types=load.test&lastEventId=", "1043");

        let reply = send_request(node.state.clone(), request).await;

        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.content_type, PROBLEM);
        assert_eq!(
            reply.json(),
            blank_problem(
                400,
                "Bad Request",
                "lastEventId must be a decimal unsigned 64-bit integer"
            )
        );
        assert_eq!(node.hub.subscriber_count(), 0);
    }
}

/// What happened to the hub of a refusing node.
#[derive(Debug, Clone, Copy)]
enum Life {
    /// Never started: the node is starting.
    Unstarted,
    /// Started at position 0.
    Started,
    /// Closed without a start.
    ClosedUnstarted,
    /// Started, then closed.
    ClosedStarted,
}

/// A node whose hub has the cap `max_subscribers` and went through `life`.
fn refusing(max_subscribers: usize, life: Life) -> Node {
    let hub = Hub::new(HubSettings {
        max_subscribers,
        buffer: 4,
    });
    if matches!(life, Life::Started | Life::ClosedStarted) {
        hub.start(Cursor::START);
    }
    if matches!(life, Life::ClosedUnstarted | Life::ClosedStarted) {
        hub.close();
    }
    let window = Arc::new(Window::new(100, usize::MAX));
    let (tail, receiver) = watch::channel(TailState::default());
    let state = state_on(
        idle_bus(),
        &receiver,
        Arc::<Window>::clone(&window),
        Arc::<Hub>::clone(&hub),
        Arc::new(StaticTopic::empty()),
    );
    Node {
        hub,
        window,
        _tail: tail,
        state,
    }
}

#[rstest]
#[case::starting(8, Life::Unstarted, "the node is starting")]
#[case::shutting_down(8, Life::ClosedStarted, "the node is shutting down")]
#[case::at_capacity(0, Life::Started, "the node is at its subscriber capacity")]
#[case::shutting_down_before_starting(0, Life::ClosedUnstarted, "the node is shutting down")]
#[case::starting_before_the_cap(0, Life::Unstarted, "the node is starting")]
#[case::shutting_down_before_the_cap(0, Life::ClosedStarted, "the node is shutting down")]
#[tokio::test(start_paused = true)]
async fn a_node_that_cannot_take_the_subscription_answers_the_503_of_fr_25(
    #[case] max_subscribers: usize,
    #[case] life: Life,
    #[case] detail: &str,
) {
    let node = refusing(max_subscribers, life);

    let reply = send(node.state.clone(), "/events/stream?types=user.created").await;

    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reply.content_type, PROBLEM);
    assert_eq!(
        reply.json(),
        json!({"type": "about:blank", "title": "Service Unavailable", "status": 503, "detail": detail})
    );
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_node_at_its_cap_counts_the_open_streams() {
    let node = node_on(
        1_000,
        Arc::new(StaticTopic::empty()),
        HubSettings {
            max_subscribers: 1,
            buffer: 4,
        },
    );
    let (first, body) = open(node.state.clone(), get("/events/stream?types=a")).await;

    let refused = send(node.state.clone(), "/events/stream?types=a").await;
    drop(body);
    let (again, _body) = open(node.state.clone(), get("/events/stream?types=a")).await;

    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(refused.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        refused.json()["detail"],
        "the node is at its subscriber capacity"
    );
    assert_eq!(again.status, StatusCode::OK);
}

#[rstest]
#[case::missing_types("lastEventId=1", StatusCode::BAD_REQUEST)]
#[case::bad_types("types=A", StatusCode::UNPROCESSABLE_ENTITY)]
#[case::bad_cursor("types=a&lastEventId=x", StatusCode::BAD_REQUEST)]
#[tokio::test(start_paused = true)]
async fn validation_comes_before_the_503_of_a_starting_node(
    #[case] query: &str,
    #[case] status: StatusCode,
) {
    let node = refusing(8, Life::Unstarted);

    let reply = send(node.state.clone(), &format!("/events/stream?{query}")).await;

    assert_eq!(reply.status, status);
}

#[tokio::test(start_paused = true)]
async fn no_problem_and_no_log_entry_holds_a_name_of_the_request() {
    let (recorder, _guard) = LogRecorder::install();
    let starting = refusing(8, Life::Unstarted);
    let replay = ScriptedTopicReplay::new();
    replay.push_page(unavailable());
    let failing = node_on(900, Arc::new(replay), HUB);

    let invalid = send(starting.state.clone(), "/events/stream?types=Secret.Name").await;
    let refused = send(starting.state.clone(), "/events/stream?types=hidden.name").await;
    let (_head, mut body) = open(
        failing.state.clone(),
        get_after("/events/stream?types=private.name", "1"),
    )
    .await;
    assert_eq!(chunk(&mut body).await, None);

    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refused.status, StatusCode::SERVICE_UNAVAILABLE);
    let lines = recorder.lines();
    assert_eq!(
        lines
            .iter()
            .map(|line| line.message.as_str())
            .collect::<Vec<_>>(),
        ["subscription rejected", "replay failed"]
    );
    let texts = [
        String::from_utf8(invalid.body).unwrap(),
        String::from_utf8(refused.body).unwrap(),
        format!("{lines:?}"),
    ];
    for text in texts {
        for name in ["Secret.Name", "hidden.name", "private.name"] {
            assert!(!text.contains(name), "{text} holds {name}");
        }
    }
}

#[tokio::test]
async fn the_stream_route_serves_only_get() {
    let reply = send_request(
        three(),
        Request::builder()
            .method(Method::POST)
            .uri("/events/stream?types=a")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
}

// ---- websocket ----

/// The `Sec-WebSocket-Key` of the test handshakes: sixteen zero bytes in base64, a valid key that
/// no secrets scanner takes for a credential.
const HANDSHAKE_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAA==";

/// The text of the rejection of a request that carries every upgrade header but no connection
/// that can be upgraded, which is what `oneshot` sends.
const NOT_UPGRADABLE: &str =
    "WebSocket request couldn't be upgraded since no upgrade state was present";

/// The upgrade headers of a WebSocket handshake.
const UPGRADE_HEADERS: [(&str, &str); 4] = [
    ("connection", "upgrade"),
    ("upgrade", "websocket"),
    ("sec-websocket-version", "13"),
    ("sec-websocket-key", HANDSHAKE_KEY),
];

/// A handshake request on `uri` with the upgrade headers except `without`.
fn handshake_without(uri: &str, without: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().uri(uri);
    for (name, value) in UPGRADE_HEADERS {
        if Some(name) != without {
            builder = builder.header(name, value);
        }
    }
    builder.body(Body::empty()).unwrap()
}

fn handshake(uri: &str) -> Request<Body> {
    handshake_without(uri, None)
}

#[test]
fn the_upgrade_headers_are_the_four_of_a_handshake() {
    let names: Vec<&str> = UPGRADE_HEADERS.iter().map(|(name, _)| *name).collect();

    assert_eq!(
        names,
        [
            CONNECTION.as_str(),
            UPGRADE.as_str(),
            SEC_WEBSOCKET_VERSION.as_str(),
            SEC_WEBSOCKET_KEY.as_str()
        ]
    );
}

#[test]
fn the_websocket_limits_are_those_of_fr_11_and_nfr_9() {
    assert_eq!(super::MAX_MESSAGE_BYTES, 32_768);
    assert_eq!(super::READ_BUFFER_BYTES, 4_096);
    assert_eq!(super::WRITE_BUFFER_BYTES, 16_384);
    assert_eq!(super::MAX_WRITE_BUFFER_BYTES, 16_384 + 1_049_600);
}

/// Tungstenite keeps its buffers to itself, so the upgrade is checked to apply each limit.
#[test]
fn the_upgrade_applies_the_message_frame_and_read_buffer_limits() {
    let source = include_str!("../handler.rs");

    assert!(source.contains(".max_message_size(MAX_MESSAGE_BYTES)"));
    assert!(source.contains(".max_frame_size(MAX_MESSAGE_BYTES)"));
    assert!(source.contains(".read_buffer_size(READ_BUFFER_BYTES)"));
    assert!(source.contains(".write_buffer_size(WRITE_BUFFER_BYTES)"));
    assert!(source.contains(".max_write_buffer_size(MAX_WRITE_BUFFER_BYTES)"));
}

#[rstest]
#[case::with_types("/events/ws?types=user.created")]
#[case::without_types("/events/ws")]
#[case::with_a_cursor("/events/ws?lastEventId=950")]
#[tokio::test(start_paused = true)]
async fn a_valid_request_that_cannot_be_upgraded_is_the_426_problem_and_frees_its_slot(
    #[case] uri: &str,
) {
    let node = ready(1_000);

    let reply = send_request(node.state.clone(), handshake(uri)).await;

    assert_eq!(reply.status, StatusCode::UPGRADE_REQUIRED);
    assert_eq!(reply.content_type, PROBLEM);
    assert_eq!(
        reply.json(),
        json!({"type": "about:blank", "title": "Upgrade Required", "status": 426, "detail": NOT_UPGRADABLE})
    );
    assert_eq!(node.hub.subscriber_count(), 0);
    assert_eq!(node.hub.type_count(), 0);
}

#[rstest]
#[case::connection("connection", "Connection header did not include 'upgrade'")]
#[case::upgrade("upgrade", "`Upgrade` header did not include 'websocket'")]
#[case::version(
    "sec-websocket-version",
    "`Sec-WebSocket-Version` header did not include '13'"
)]
#[case::key("sec-websocket-key", "`Sec-WebSocket-Key` header missing")]
#[tokio::test(start_paused = true)]
async fn a_request_that_is_not_a_valid_upgrade_is_the_400_problem_of_its_rejection(
    #[case] without: &str,
    #[case] detail: &str,
) {
    let node = ready(1_000);

    let reply = send_request(
        node.state.clone(),
        handshake_without("/events/ws?types=user.created", Some(without)),
    )
    .await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.content_type, PROBLEM);
    assert_eq!(
        reply.json(),
        json!({"type": "about:blank", "title": "Bad Request", "status": 400, "detail": detail})
    );
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[rstest]
#[case::empty("types=", "required")]
#[case::empty_entry("types=a,,b", "required")]
#[case::pattern("types=User.Created", "pattern")]
#[case::length(&format!("types={}", "a".repeat(129)), "length")]
#[case::max_items(&format!("types={}", distinct_names(101)), "max_items")]
#[case::before_a_bad_cursor("types=&lastEventId=abc", "required")]
#[tokio::test(start_paused = true)]
async fn invalid_types_on_the_websocket_route_are_422_before_the_503_and_the_upgrade(
    #[case] query: &str,
    #[case] code: &str,
) {
    let node = refusing(8, Life::Unstarted);

    let reply = send(node.state.clone(), &format!("/events/ws?{query}")).await;

    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.content_type, PROBLEM);
    let body = reply.json();
    let violations = body["violations"].as_array().unwrap();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["propertyPath"], "types");
    assert_eq!(violations[0]["code"], code);
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[rstest]
#[case::empty("")]
#[case::text("abc")]
#[case::negative("-1")]
#[case::overflow("18446744073709551616")]
#[tokio::test(start_paused = true)]
async fn an_invalid_cursor_on_the_websocket_route_is_400_naming_its_carrier(#[case] value: &str) {
    let node = refusing(8, Life::Unstarted);
    let mut by_header = handshake("/events/ws?types=a&lastEventId=5");
    by_header
        .headers_mut()
        .insert("last-event-id", value.parse().unwrap());
    let mut beside_a_valid_header = handshake(&format!("/events/ws?types=a&lastEventId={value}"));
    beside_a_valid_header
        .headers_mut()
        .insert("last-event-id", HeaderValue::from_static("1043"));

    let by_header = send_request(node.state.clone(), by_header).await;
    let by_parameter = send(
        node.state.clone(),
        &format!("/events/ws?lastEventId={value}"),
    )
    .await;
    let beside_a_valid_header = send_request(node.state.clone(), beside_a_valid_header).await;

    for (reply, carrier) in [
        (by_header, "Last-Event-ID"),
        (by_parameter, "lastEventId"),
        (beside_a_valid_header, "lastEventId"),
    ] {
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{carrier}");
        assert_eq!(reply.content_type, PROBLEM);
        assert_eq!(
            reply.json()["detail"],
            format!("{carrier} must be a decimal unsigned 64-bit integer")
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_websocket_parameter_that_cannot_be_read_is_400_naming_it() {
    let node = ready(1_000);

    let reply = send(node.state.clone(), "/events/ws?types=a&types=b").await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.content_type, PROBLEM);
    let detail = reply.json()["detail"].as_str().unwrap().to_owned();
    assert!(detail.contains("`types`"), "{detail}");
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[rstest]
#[case::starting(8, Life::Unstarted, "the node is starting")]
#[case::shutting_down(8, Life::ClosedStarted, "the node is shutting down")]
#[case::at_capacity(0, Life::Started, "the node is at its subscriber capacity")]
#[case::shutting_down_before_starting(0, Life::ClosedUnstarted, "the node is shutting down")]
#[case::starting_before_the_cap(0, Life::Unstarted, "the node is starting")]
#[tokio::test(start_paused = true)]
async fn a_node_that_cannot_take_the_websocket_answers_the_503_before_the_upgrade(
    #[case] max_subscribers: usize,
    #[case] life: Life,
    #[case] detail: &str,
) {
    let node = refusing(max_subscribers, life);

    let valid = send_request(node.state.clone(), handshake("/events/ws?types=a")).await;
    let not_an_upgrade = send(node.state.clone(), "/events/ws?types=a").await;

    for reply in [valid, not_an_upgrade] {
        assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(reply.content_type, PROBLEM);
        assert_eq!(
            reply.json(),
            json!({"type": "about:blank", "title": "Service Unavailable", "status": 503, "detail": detail})
        );
    }
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn the_websocket_route_counts_against_the_cap_of_the_open_streams() {
    let node = node_on(
        1_000,
        Arc::new(StaticTopic::empty()),
        HubSettings {
            max_subscribers: 1,
            buffer: 4,
        },
    );
    let (_head, _body) = open(node.state.clone(), get("/events/stream?types=a")).await;

    let refused = send_request(node.state.clone(), handshake("/events/ws?types=a")).await;

    assert_eq!(refused.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        refused.json()["detail"],
        "the node is at its subscriber capacity"
    );
}

#[tokio::test(start_paused = true)]
async fn no_websocket_problem_and_no_log_entry_holds_a_name_of_the_request() {
    let (recorder, _guard) = LogRecorder::install();
    let starting = refusing(8, Life::Unstarted);
    let ready = ready(1_000);

    let invalid = send(starting.state.clone(), "/events/ws?types=Secret.Name").await;
    let refused = send(starting.state.clone(), "/events/ws?types=hidden.name").await;
    let rejected = send(ready.state.clone(), "/events/ws?types=private.name").await;

    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refused.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(rejected.status, StatusCode::BAD_REQUEST);
    let lines = recorder.lines();
    let texts = [
        String::from_utf8(invalid.body).unwrap(),
        String::from_utf8(refused.body).unwrap(),
        String::from_utf8(rejected.body).unwrap(),
        format!("{lines:?}"),
    ];
    for text in texts {
        for name in ["Secret.Name", "hidden.name", "private.name"] {
            assert!(!text.contains(name), "{text} holds {name}");
        }
    }
}

#[tokio::test]
async fn the_websocket_route_serves_only_get() {
    let reply = send_request(
        three(),
        Request::builder()
            .method(Method::POST)
            .uri("/events/ws?types=a")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
}

// ---- documentation ----

fn document() -> Value {
    serde_json::to_value(router::<EventStreamState>().into_openapi()).unwrap()
}

/// The `default` answer of every operation: the 500 problem of the panic catcher as its example.
fn default_response() -> Value {
    problem_with_example(
        "Problem response.",
        &json!({"type": "about:blank", "title": "Internal Server Error", "status": 500}),
    )
}

#[test]
fn the_document_has_the_four_operations_of_the_event_stream() {
    let document = document();

    let paths = document["paths"].as_object().unwrap();
    let mut names: Vec<&str> = paths.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "/event-types",
            "/events/status",
            "/events/stream",
            "/events/ws"
        ]
    );
    for path in names {
        assert_eq!(
            paths[path].as_object().unwrap().keys().collect::<Vec<_>>(),
            ["get"],
            "{path}"
        );
    }
}

/// The problem response of `status` with its description and example.
fn problem_with_example(description: &str, example: &Value) -> Value {
    json!({
        "description": description,
        "content": {"application/problem+json": {
            "schema": {"$ref": "#/components/schemas/Problem"},
            "example": example,
        }}
    })
}

fn blank_problem(status: u16, title: &str, detail: &str) -> Value {
    json!({"type": "about:blank", "title": title, "status": status, "detail": detail})
}

fn types_problem_example() -> Value {
    json!({
        "type": "/problems/validation-failed",
        "title": "Unprocessable Entity",
        "status": 422,
        "detail": "The request failed validation.",
        "violations": [{
            "propertyPath": "types",
            "message": "types must not hold an empty event type name",
            "code": "required",
        }],
    })
}

fn parameter_names(operation: &Value) -> Vec<(String, String, bool)> {
    operation["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| {
            (
                parameter["name"].as_str().unwrap().to_owned(),
                parameter["in"].as_str().unwrap().to_owned(),
                parameter["required"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn named(names: &[(&str, &str, bool)]) -> Vec<(String, String, bool)> {
    names
        .iter()
        .map(|(name, location, required)| ((*name).to_owned(), (*location).to_owned(), *required))
        .collect()
}

fn status_codes(operation: &Value) -> Vec<&str> {
    operation["responses"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn the_stream_operation_documents_its_parameters_and_every_answer() {
    let document = document();

    let operation = &document["paths"]["/events/stream"]["get"];
    assert_eq!(operation["operationId"], "api_events_stream_get");
    assert_eq!(operation["tags"], json!(["EventStream"]));
    assert_eq!(
        operation["summary"],
        "Opens an event stream of the listed event types."
    );
    assert!(
        operation["description"]
            .as_str()
            .unwrap()
            .contains("Server-Sent Events")
    );
    assert_eq!(
        parameter_names(operation),
        named(&[
            ("types", "query", true),
            ("lastEventId", "query", false),
            ("Last-Event-ID", "header", false),
        ])
    );
    let responses = &operation["responses"];
    assert_eq!(
        status_codes(operation),
        ["200", "400", "422", "503", "default"]
    );
    let opened = &responses["200"];
    assert_eq!(opened["description"], super::STREAM_OPENED);
    assert_eq!(
        opened["content"],
        json!({"text/event-stream": {
            "schema": {"$ref": "#/components/schemas/SseFrame"},
            "example": {"event": "load.test", "id": "1043", "data": super::EXAMPLE_EVENT},
        }})
    );
    assert_eq!(
        opened["headers"],
        json!({
            "cache-control": {"schema": {"type": "string"}, "description": "Always `no-cache`."},
            "x-accel-buffering": {"schema": {"type": "string"}, "description": "Always `no`, so that a proxy does not hold frames and keep-alives back."},
        })
    );
    assert_eq!(
        responses["400"],
        problem_with_example(
            super::STREAM_BAD_REQUEST,
            &blank_problem(
                400,
                "Bad Request",
                "lastEventId must be a decimal unsigned 64-bit integer"
            )
        )
    );
    assert_eq!(
        responses["422"],
        problem_with_example(super::TYPES_INVALID, &types_problem_example())
    );
    assert_eq!(
        responses["503"],
        problem_with_example(
            super::NODE_UNAVAILABLE,
            &blank_problem(503, "Service Unavailable", "the node is starting")
        )
    );
    assert_eq!(responses["default"], default_response());
}

#[test]
fn the_websocket_operation_documents_its_handshake_its_frames_and_every_answer() {
    let document = document();

    let operation = &document["paths"]["/events/ws"]["get"];
    assert_eq!(operation["operationId"], "api_events_ws_get");
    assert_eq!(operation["tags"], json!(["EventStream"]));
    assert_eq!(
        operation["summary"],
        "Upgrades to a WebSocket subscription of the listed event types."
    );
    assert!(operation["description"].as_str().unwrap().contains("(403)"));
    assert_eq!(
        parameter_names(operation),
        named(&[
            ("types", "query", false),
            ("lastEventId", "query", false),
            ("Connection", "header", true),
            ("Upgrade", "header", true),
            ("Sec-WebSocket-Version", "header", true),
            ("Sec-WebSocket-Key", "header", true),
            ("Last-Event-ID", "header", false),
        ])
    );
    let responses = &operation["responses"];
    assert_eq!(
        status_codes(operation),
        ["101", "400", "403", "422", "426", "503", "default"]
    );
    assert_eq!(
        responses["101"],
        json!({
            "description": super::WS_UPGRADED,
            "headers": {
                "Connection": {"schema": {"type": "string"}, "description": "`upgrade`."},
                "Sec-WebSocket-Accept": {"schema": {"type": "string"}, "description": "The key derived from `Sec-WebSocket-Key` as RFC 6455 states."},
                "Upgrade": {"schema": {"type": "string"}, "description": "`websocket`."},
            },
        })
    );
    let server_frame = format!(
        r#"{{"id":"{EXAMPLE_CURSOR}","type":"{EXAMPLE_TYPE}","data":{}}}"#,
        super::EXAMPLE_EVENT
    );
    for part in [
        server_frame.as_str(),
        r#"{"error":"invalid command"}"#,
        r#"{"action":"subscribe","types":["load.test"]}"#,
        r#"{"action":"unsubscribe","types":["load.test"]}"#,
    ] {
        assert!(super::WS_UPGRADED.contains(&format!("`{part}`")), "{part}");
    }
    assert_eq!(
        responses["400"],
        problem_with_example(
            super::WS_BAD_REQUEST,
            &blank_problem(
                400,
                "Bad Request",
                "Connection header did not include 'upgrade'"
            )
        )
    );
    assert_eq!(
        responses["403"],
        problem_with_example(
            super::ORIGIN_REFUSED,
            &blank_problem(403, "Forbidden", "the origin is not allowed")
        )
    );
    assert_eq!(
        responses["422"],
        problem_with_example(super::TYPES_INVALID, &types_problem_example())
    );
    assert_eq!(
        responses["426"],
        problem_with_example(
            super::NOT_UPGRADABLE_TEXT,
            &blank_problem(426, "Upgrade Required", NOT_UPGRADABLE)
        )
    );
    assert_eq!(
        responses["503"],
        problem_with_example(
            super::NODE_UNAVAILABLE,
            &blank_problem(503, "Service Unavailable", "the node is starting")
        )
    );
    assert_eq!(responses["default"], default_response());
}

#[test]
fn the_websocket_handshake_problems_name_their_causes() {
    assert!(super::STREAM_BAD_REQUEST.contains("cursor"));
    assert!(super::WS_BAD_REQUEST.contains("handshake header"));
    assert!(super::ORIGIN_REFUSED.contains("CORS_ALLOWED_ORIGINS"));
    assert!(super::TYPES_INVALID.contains("`max_items`"));
    assert!(super::NODE_UNAVAILABLE.contains("safe to retry"));
    assert!(super::NOT_UPGRADABLE_TEXT.contains("cannot be upgraded"));
    assert!(super::STREAM_OPENED.contains("`:`"));
}

/// The parameter of the operation at `path` named `name`, its name compared without case.
fn parameter<'a>(document: &'a Value, path: &str, name: &str) -> &'a Value {
    document["paths"][path]["get"]["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|parameter| {
            parameter["name"]
                .as_str()
                .unwrap()
                .eq_ignore_ascii_case(name)
        })
        .unwrap_or_else(|| panic!("{path} documents {name}"))
}

/// Whether the parameter admits the value as it is sent: a list that is not exploded is split at
/// its commas, every other value is one string; an absent value is admitted when not required.
fn admits_sent(document: &Value, parameter: &Value, sent: Option<&str>) -> bool {
    let Some(text) = sent else {
        return parameter["required"] == false;
    };
    let value = if parameter["explode"] == false {
        json!(text.split(',').collect::<Vec<_>>())
    } else {
        json!(text)
    };
    json_schema::admits(document, &parameter["schema"], &value)
}

/// The `types` values the tests of the streaming routes send, accepted or refused by the routes.
fn types_sent() -> Vec<(String, bool)> {
    let mut sent: Vec<(String, bool)> = [
        "user.created",
        "order.paid,user.created,order.paid",
        "a",
        "b,a,b,a",
        "load.test",
        "first.ok",
    ]
    .iter()
    .map(|text| ((*text).to_owned(), true))
    .collect();
    sent.push(("a".repeat(128), true));
    sent.push((distinct_names(100), true));
    for refused in [
        "",
        "a,,b",
        "a,",
        "User.Created",
        "A",
        "B,",
        " ",
        "Secret.Name",
        "first.ok,,Secret.Name",
    ] {
        sent.push((refused.to_owned(), false));
    }
    sent.push(("a".repeat(129), false));
    sent.push(("x".repeat(129), false));
    sent.push((distinct_names(101), false));
    sent
}

#[test]
fn a_types_value_matches_the_documented_schema_exactly_when_the_routes_accept_it() {
    let document = document();

    for (text, accepted) in types_sent() {
        assert_eq!(
            parse_types(&text).is_ok(),
            accepted,
            "the route on {text:?}"
        );
        for path in ["/events/stream", "/events/ws"] {
            let types = parameter(&document, path, "types");
            assert_eq!(
                admits_sent(&document, types, Some(&text)),
                accepted,
                "{path} on {text:?}"
            );
        }
    }
    assert!(!admits_sent(
        &document,
        parameter(&document, "/events/stream", "types"),
        None
    ));
    assert!(admits_sent(
        &document,
        parameter(&document, "/events/ws", "types"),
        None
    ));
}

#[test]
fn a_repeated_name_is_the_one_types_fault_the_schema_cannot_express_and_its_text_says_so() {
    let document = document();
    let repeated = format!("{0},{0}", distinct_names(100));

    assert!(parse_types(&repeated).is_ok());
    for path in ["/events/stream", "/events/ws"] {
        let types = parameter(&document, path, "types");
        assert!(!admits_sent(&document, types, Some(&repeated)));
        let text = types["description"].as_str().unwrap();
        assert!(text.contains("A repeated name counts once"), "{text}");
        assert!(text.contains("cannot express"), "{text}");
    }
}

/// The cursor values the tests send, accepted or refused by the routes.
const CURSORS_SENT: [(&str, bool); 21] = [
    ("0", true),
    ("1", true),
    ("2", true),
    ("3", true),
    ("5", true),
    ("7", true),
    ("8", true),
    ("100", true),
    ("950", true),
    ("007", true),
    ("18446744073709551615", true),
    ("abc", false),
    ("x", false),
    ("-1", false),
    ("+1", false),
    ("1 2", false),
    (" 1", false),
    ("1.5", false),
    ("", false),
    ("18446744073709551616", false),
    ("\u{ff}1", false),
];

#[test]
fn a_cursor_matches_the_documented_schemas_exactly_when_the_routes_accept_it() {
    let document = document();

    for (text, accepted) in CURSORS_SENT {
        assert_eq!(
            read_cursor(None, Some(text)).is_ok(),
            accepted,
            "the parameter rule on {text:?}"
        );
        if let Ok(header) = text.parse::<HeaderValue>() {
            assert_eq!(
                read_cursor(Some(&header), None).is_ok(),
                accepted,
                "the header rule on {text:?}"
            );
        }
        for path in ["/events/stream", "/events/ws"] {
            for name in ["lastEventId", "Last-Event-ID"] {
                let cursor = parameter(&document, path, name);
                assert_eq!(
                    admits_sent(&document, cursor, Some(text)),
                    accepted,
                    "{path} {name} on {text:?}"
                );
                assert!(admits_sent(&document, cursor, None));
            }
        }
    }
}

/// The values around each digit of the largest cursor, 18446744073709551615: at each position the
/// digit one lower followed by nines, the largest cursor with that prefix, and the digit one higher
/// followed by zeros, the smallest value with that prefix above the largest cursor; then each of
/// them, the largest cursor included, after a leading zero. `true` for a cursor.
fn cursors_around_the_largest() -> Vec<(String, bool)> {
    let largest: Vec<u32> = u64::MAX
        .to_string()
        .chars()
        .map(|digit| digit.to_digit(10).unwrap())
        .collect();
    let mut values = vec![(u64::MAX.to_string(), true)];
    for (position, digit) in largest.iter().enumerate() {
        // The digits of the largest cursor before `position`, `at` at it and `fill` after it.
        let around = |at: u32, fill: u32| -> String {
            largest
                .iter()
                .enumerate()
                .map(|(index, other)| match index.cmp(&position) {
                    Ordering::Less => *other,
                    Ordering::Equal => at,
                    Ordering::Greater => fill,
                })
                .map(|digit| char::from_digit(digit, 10).unwrap())
                .collect()
        };
        if let Some(lower) = digit.checked_sub(1) {
            values.push((around(lower, 9), true));
        }
        if let Some(higher) = digit.checked_add(1).filter(|higher| *higher <= 9) {
            values.push((around(higher, 0), false));
        }
    }
    let padded: Vec<(String, bool)> = values
        .iter()
        .map(|(text, accepted)| (format!("0{text}"), *accepted))
        .collect();
    values.extend(padded);
    values
}

#[test]
fn the_values_around_the_largest_cursor_are_one_per_digit_bound() {
    let values = cursors_around_the_largest();

    // 20 digits, of which two are 0 (no lower value) and one is 9 (no higher value).
    assert_eq!(values.len(), 2 * (1 + 18 + 19));
    assert!(values.contains(&("09999999999999999999".to_owned(), true)));
    assert!(values.contains(&("20000000000000000000".to_owned(), false)));
    assert!(values.contains(&("18446744073709551614".to_owned(), true)));
    assert!(values.contains(&("18446744073709551616".to_owned(), false)));
    assert!(values.contains(&("018446744073709551615".to_owned(), true)));
    assert!(values.contains(&("018446744074000000000".to_owned(), false)));
}

#[test]
fn a_cursor_next_to_a_digit_bound_matches_the_pattern_exactly_when_the_routes_accept_it() {
    let document = document();

    for (text, accepted) in cursors_around_the_largest() {
        assert_eq!(
            Cursor::parse(&text).is_ok(),
            accepted,
            "the cursor on {text}"
        );
        assert_eq!(
            read_cursor(None, Some(&text)).is_ok(),
            accepted,
            "the parameter rule on {text}"
        );
        for path in ["/events/stream", "/events/ws"] {
            for name in ["lastEventId", "Last-Event-ID"] {
                let pattern = parameter(&document, path, name)["schema"]["pattern"]
                    .as_str()
                    .unwrap();
                assert_eq!(
                    json_schema::is_match(pattern, &text),
                    accepted,
                    "{path} {name} on {text}"
                );
            }
        }
    }
}

/// The handshake header values sent to the WebSocket route; `true` when the upgrade accepts it.
const HANDSHAKES_SENT: [(&str, &str, bool); 13] = [
    ("connection", "upgrade", true),
    ("connection", "Upgrade", true),
    ("connection", "keep-alive, Upgrade", true),
    ("connection", "keep-alive", false),
    ("upgrade", "websocket", true),
    ("upgrade", "WebSocket", true),
    ("upgrade", "h2c", false),
    ("upgrade", "websocket2", false),
    ("sec-websocket-version", "13", true),
    ("sec-websocket-version", "8", false),
    ("sec-websocket-version", "130", false),
    ("sec-websocket-key", HANDSHAKE_KEY, true),
    ("sec-websocket-key", "", true),
];

/// A handshake on the WebSocket route with `name` set to `value`, or without it.
fn handshake_with(name: &str, value: Option<&str>) -> Request<Body> {
    let mut request = handshake_without("/events/ws?types=user.created", Some(name));
    if let Some(value) = value {
        request.headers_mut().insert(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }
    request
}

#[tokio::test(start_paused = true)]
async fn a_handshake_header_matches_the_documented_schema_exactly_when_the_upgrade_accepts_it() {
    let document = document();
    let node = ready(1_000);

    for (name, value, accepted) in HANDSHAKES_SENT {
        let reply = send_request(node.state.clone(), handshake_with(name, Some(value))).await;

        let expected = if accepted {
            StatusCode::UPGRADE_REQUIRED
        } else {
            StatusCode::BAD_REQUEST
        };
        assert_eq!(reply.status, expected, "the route on {name}: {value:?}");
        let header = parameter(&document, "/events/ws", name);
        assert_eq!(
            admits_sent(&document, header, Some(value)),
            accepted,
            "{name}: {value:?}"
        );
    }
    for (name, _) in UPGRADE_HEADERS {
        let reply = send_request(node.state.clone(), handshake_with(name, None)).await;

        assert_eq!(
            reply.status,
            StatusCode::BAD_REQUEST,
            "the route without {name}"
        );
        let header = parameter(&document, "/events/ws", name);
        assert!(!admits_sent(&document, header, None), "{name}");
    }
    assert_eq!(node.hub.subscriber_count(), 0);
}

/// The fields of one SSE frame as an object: `field: value` per line.
fn frame_fields(frame: &str) -> Value {
    let mut fields = serde_json::Map::new();
    for line in frame.lines().filter(|line| !line.is_empty()) {
        let (field, value) = line.split_once(": ").unwrap();
        fields.insert(field.to_owned(), json!(value));
    }
    Value::Object(fields)
}

#[tokio::test(start_paused = true)]
async fn an_event_frame_of_the_route_matches_the_frame_schema_and_its_data_the_event_object() {
    let document = document();
    let frame_schema = json!({"$ref": "#/components/schemas/SseFrame"});
    let node = ready(1_000);
    let (_head, mut body) =
        open(node.state.clone(), get("/events/stream?types=user.created")).await;

    node.pass(&event(1_001, "user.created"));
    let written = chunk(&mut body).await.unwrap();

    let fields = frame_fields(&written);
    assert_eq!(
        fields,
        json!({"id": "1001", "event": "user.created", "data": event_json(1_001, "user.created")})
    );
    assert!(json_schema::admits(&document, &frame_schema, &fields));
    let content =
        &document["components"]["schemas"]["SseFrame"]["properties"]["data"]["contentSchema"];
    let data: Value = serde_json::from_str(fields["data"].as_str().unwrap()).unwrap();
    assert!(json_schema::admits(&document, content, &data));
}

#[rstest]
#[case::id_zero(json!({"id": "0", "event": "a", "data": event_json(1, "a")}))]
#[case::id_padded(json!({"id": "01", "event": "a", "data": event_json(1, "a")}))]
#[case::event_not_a_name(json!({"id": "1", "event": "A", "data": event_json(1, "a")}))]
#[case::no_event(json!({"id": "1", "data": event_json(1, "a")}))]
#[case::no_id(json!({"event": "a", "data": event_json(1, "a")}))]
#[case::no_data(json!({"id": "1", "event": "a"}))]
#[case::data_not_json(json!({"id": "1", "event": "a", "data": "{"}))]
#[case::data_without_a_member(json!({"id": "1", "event": "a", "data": r#"{"id":"00000000-0000-0000-0000-000000000001","type":"a","data":1}"#}))]
#[case::data_with_another_member(json!({"id": "1", "event": "a", "data": r#"{"id":"00000000-0000-0000-0000-000000000001","type":"a","occurredAt":"2026-11-14T10:00:00Z","data":1,"key":"k"}"#}))]
#[case::data_with_a_bad_type(json!({"id": "1", "event": "a", "data": r#"{"id":"00000000-0000-0000-0000-000000000001","type":"A","occurredAt":"2026-11-14T10:00:00Z","data":1}"#}))]
fn a_frame_the_route_never_writes_does_not_match_the_frame_schema(#[case] fields: Value) {
    let document = document();

    assert!(!json_schema::admits(
        &document,
        &json!({"$ref": "#/components/schemas/SseFrame"}),
        &fields
    ));
}

#[test]
fn every_example_of_the_event_stream_operations_matches_its_schema() {
    let document = document();

    for path in [
        "/event-types",
        "/events/status",
        "/events/stream",
        "/events/ws",
    ] {
        let operation = &document["paths"][path]["get"];
        for parameter in operation["parameters"].as_array().into_iter().flatten() {
            assert!(
                json_schema::admits(&document, &parameter["schema"], &parameter["example"]),
                "{path} {}",
                parameter["name"]
            );
        }
        for (status, response) in operation["responses"].as_object().unwrap() {
            for (media_type, content) in response["content"].as_object().into_iter().flatten() {
                if let Some(example) = content.get("example") {
                    assert!(
                        json_schema::admits(&document, &content["schema"], example),
                        "{path} {status} {media_type}"
                    );
                }
            }
        }
    }
    let frame = &document["components"]["schemas"]["SseFrame"]["properties"];
    for field in ["event", "id", "data"] {
        let examples = frame[field]["examples"].as_array().unwrap();
        assert_eq!(examples.len(), 1, "{field}");
        for example in examples {
            assert!(
                json_schema::admits(&document, &frame[field], example),
                "{field}"
            );
        }
    }
}

/// The example of the problem answer `status` of the operation at `path`.
fn problem_example<'a>(document: &'a Value, path: &str, status: &str) -> &'a Value {
    &document["paths"][path]["get"]["responses"][status]["content"][PROBLEM]["example"]
}

#[tokio::test(start_paused = true)]
async fn every_problem_example_is_the_answer_of_its_producer() {
    let document = document();
    let node = ready(1_000);
    let answers = [
        (
            "/event-types",
            "400",
            send(three(), "/event-types?page=abc").await,
        ),
        (
            "/event-types",
            "422",
            send(three(), "/event-types?itemsPerPage=101").await,
        ),
        (
            "/events/stream",
            "400",
            send(three(), "/events/stream?types=a&lastEventId=x").await,
        ),
        (
            "/events/stream",
            "422",
            send(three(), "/events/stream?types=a,,b").await,
        ),
        (
            "/events/stream",
            "503",
            send(three(), "/events/stream?types=a").await,
        ),
        (
            "/events/ws",
            "400",
            send_request(
                node.state.clone(),
                handshake_with("connection", Some("keep-alive")),
            )
            .await,
        ),
        (
            "/events/ws",
            "422",
            send_request(three(), handshake("/events/ws?types=a,,b")).await,
        ),
        (
            "/events/ws",
            "426",
            send_request(node.state.clone(), handshake("/events/ws?types=a")).await,
        ),
        (
            "/events/ws",
            "503",
            send_request(three(), handshake("/events/ws?types=a")).await,
        ),
    ];

    for (path, status, reply) in answers {
        assert_eq!(reply.status.as_str(), status, "{path} {status}");
        assert_eq!(reply.content_type, PROBLEM, "{path} {status}");
        assert_eq!(
            *problem_example(&document, path, status),
            reply.json(),
            "{path} {status}"
        );
    }
    let types = serde_json::to_value(types_problem(TypesError::Blank)).unwrap();
    let unavailable = serde_json::to_value(Problem::from(HubError::NotReady)).unwrap();
    for path in ["/events/stream", "/events/ws"] {
        assert_eq!(*problem_example(&document, path, "422"), types, "{path}");
        assert_eq!(
            *problem_example(&document, path, "503"),
            unavailable,
            "{path}"
        );
    }
    let cursor = parse_cursor(&HeaderMap::new(), Some("x")).unwrap_err();
    assert_eq!(
        *problem_example(&document, "/events/stream", "400"),
        serde_json::to_value(cursor).unwrap()
    );
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test]
async fn the_catalog_example_is_the_answer_of_a_catalog_of_its_names() {
    let document = document();

    let reply = send(state(&["user.created", EXAMPLE_TYPE]), "/event-types").await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        document["paths"]["/event-types"]["get"]["responses"]["200"]["content"]["application/json"]
            ["example"],
        reply.json()
    );
}

/// The event of the examples, as the tail hands it to the hub.
fn example_event() -> StoredEvent {
    stored_event(
        EXAMPLE_CURSOR.parse().unwrap(),
        EXAMPLE_TYPE,
        super::EXAMPLE_EVENT,
    )
}

#[tokio::test(start_paused = true)]
async fn the_frame_examples_are_the_frames_the_routes_write() {
    let document = document();
    let node = ready(1_000);
    let (_head, mut body) = open(
        node.state.clone(),
        get(&format!("/events/stream?types={EXAMPLE_TYPE}")),
    )
    .await;

    node.pass(&example_event());
    let written = chunk(&mut body).await.unwrap();

    assert_eq!(
        document["paths"]["/events/stream"]["get"]["responses"]["200"]["content"]["text/event-stream"]
            ["example"],
        frame_fields(&written)
    );
    let frame = &document["components"]["schemas"]["SseFrame"]["properties"];
    assert_eq!(frame["event"]["examples"], json!([EXAMPLE_TYPE]));
    assert_eq!(frame["id"]["examples"], json!([EXAMPLE_CURSOR]));
    assert_eq!(frame["data"]["examples"], json!([super::EXAMPLE_EVENT]));
    let server_frame = ws_json(&example_event());
    assert_eq!(
        server_frame,
        format!(
            r#"{{"id":"{EXAMPLE_CURSOR}","type":"{EXAMPLE_TYPE}","data":{}}}"#,
            super::EXAMPLE_EVENT
        )
    );
    assert!(super::WS_UPGRADED.contains(&format!("`{server_frame}`")));
}

#[test]
fn the_documented_messages_of_a_websocket_are_those_of_its_session() {
    for (text, command) in [
        (
            r#"{"action":"subscribe","types":["load.test"]}"#,
            Command::Subscribe {
                types: vec![EXAMPLE_TYPE.to_owned()],
            },
        ),
        (
            r#"{"action":"unsubscribe","types":["load.test"]}"#,
            Command::Unsubscribe {
                types: vec![EXAMPLE_TYPE.to_owned()],
            },
        ),
    ] {
        assert!(super::WS_UPGRADED.contains(&format!("`{text}`")), "{text}");
        assert_eq!(serde_json::from_str::<Command>(text).unwrap(), command);
    }
    let error = error_json("invalid command");
    assert!(
        super::WS_UPGRADED.contains(&format!("`{error}`")),
        "{error}"
    );
}

/// `value` in decimal with a space before each group of three digits from the right, as the
/// descriptions write a number.
fn grouped(value: usize) -> String {
    let reversed: Vec<char> = value.to_string().chars().rev().collect();
    let groups: Vec<String> = reversed
        .chunks(3)
        .rev()
        .map(|group| group.iter().rev().collect())
        .collect();
    groups.join(" ")
}

#[test]
fn the_texts_of_the_streaming_operations_state_the_limits_of_the_code() {
    assert_eq!(grouped(7), "7");
    assert_eq!(grouped(100), "100");
    assert_eq!(grouped(4_096), "4 096");
    assert_eq!(grouped(1_234_567), "1 234 567");
    for part in [
        format!(
            "a frame larger than {} bytes ends the connection",
            grouped(super::MAX_MESSAGE_BYTES)
        ),
        format!(
            "Text frames of at most {} bytes",
            grouped(Session::MAX_COMMAND_BYTES)
        ),
        format!("each with 1 to {} distinct names", Subscription::MAX_TYPES),
    ] {
        assert!(super::WS_UPGRADED.contains(&part), "{part}");
    }
    let too_many = format!("more than {} distinct names", Subscription::MAX_TYPES);
    assert!(super::TYPES_INVALID.contains(&too_many));
}

#[test]
fn the_status_operation_matches_the_contract() {
    let document = document();

    let operation = &document["paths"]["/events/status"]["get"];
    assert_eq!(operation["operationId"], "api_events_status_get");
    assert_eq!(operation["tags"], json!(["EventStream"]));
    assert_eq!(operation["summary"], "Retrieves the event stream status.");
    assert!(
        operation["description"]
            .as_str()
            .unwrap()
            .contains("answers in every life stage")
    );
    assert!(operation.get("parameters").is_none());
    let responses = &operation["responses"];
    assert_eq!(
        responses["200"]["description"],
        "EventStreamStatus resource"
    );
    assert_eq!(
        responses["200"]["content"]["application/json"]["schema"],
        json!({"$ref": "#/components/schemas/EventStreamStatus"})
    );
    let example = &responses["200"]["content"]["application/json"]["example"];
    let mut fields: Vec<&str> = example
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "brokerReachable",
            "endOffset",
            "lag",
            "position",
            "reason",
            "rejectedRecords",
            "replayCapped",
            "retentionLosses",
            "state",
            "subscribers",
            "windowFirst",
            "windowLast"
        ]
    );
    assert_eq!(example["state"], "ready");
    assert_eq!(responses["default"], default_response());
    assert_eq!(
        responses.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["200", "default"]
    );
}

#[test]
fn the_catalog_operation_matches_the_contract() {
    let document = document();

    let operation = &document["paths"]["/event-types"]["get"];
    assert_eq!(operation["operationId"], "api_event_types_get");
    assert_eq!(operation["tags"], json!(["EventStream"]));
    assert_eq!(
        operation["summary"],
        "Retrieves the collection of EventType resources."
    );
    let parameters = operation["parameters"].as_array().unwrap();
    let names: Vec<&str> = parameters
        .iter()
        .map(|parameter| parameter["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["page", "itemsPerPage"]);
    assert_eq!(parameters[0]["in"], "query");
    assert_eq!(parameters[0]["schema"]["minimum"], 1);
    assert_eq!(parameters[0]["schema"]["default"], 1);
    assert_eq!(parameters[1]["schema"]["minimum"], 1);
    assert_eq!(parameters[1]["schema"]["maximum"], 100);
    assert_eq!(parameters[1]["schema"]["default"], 30);
    assert_eq!(parameters[0]["example"], 1);
    assert_eq!(parameters[1]["example"], 10);
    let responses = &operation["responses"];
    assert_eq!(
        responses["200"]["content"]["application/json"],
        json!({
            "schema": {"type": "array", "items": {"$ref": "#/components/schemas/EventType"}},
            "example": [{"name": "load.test"}, {"name": "user.created"}],
        })
    );
    assert_eq!(
        responses["400"],
        problem_with_example(
            "Invalid query parameter.",
            &blank_problem(
                400,
                "Bad Request",
                "Failed to deserialize query string: page: invalid digit found in string"
            )
        )
    );
    assert_eq!(
        responses["422"],
        problem_with_example(
            "Validation failed.",
            &json!({
                "type": "/problems/validation-failed",
                "title": "Unprocessable Entity",
                "status": 422,
                "detail": "The request failed validation.",
                "violations": [{
                    "propertyPath": "itemsPerPage",
                    "message": "itemsPerPage must be between 1 and 100",
                    "code": "range",
                }],
            })
        )
    );
    assert_eq!(responses["default"], default_response());
    assert_eq!(
        responses.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["200", "400", "422", "default"]
    );
}

#[test]
fn the_document_carries_the_event_type_and_the_frame_schemas() {
    let document = document();

    let schemas = &document["components"]["schemas"];
    for name in ["EventStreamStatus", "EventType", "Problem", "SseFrame"] {
        assert!(schemas.get(name).is_some(), "{name} is missing");
    }
}

// ---- extractor check ----

/// The extractor types a handler of this module may take. `HeaderMap` and the result of the
/// upgrade extractor are infallible: the recorded exception of the streaming routes.
const ALLOWED_EXTRACTORS: [&str; 4] = [
    "State<EventStreamState>",
    "ValidatedQuery<",
    "HeaderMap",
    "Result<WebSocketUpgrade, WebSocketUpgradeRejection>",
];

/// Splits `text` at every `separator` that is not nested in brackets.
fn split_top_level(text: &str, separator: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0_usize;
    let mut skip = 0_usize;
    for (index, character) in text.char_indices() {
        if skip > 0 {
            skip = skip.saturating_sub(1);
            continue;
        }
        let at_separator = depth == 0
            && text
                .get(index..)
                .is_some_and(|rest| rest.starts_with(separator));
        if at_separator {
            parts.push(std::mem::take(&mut current));
            skip = separator.chars().count().saturating_sub(1);
            continue;
        }
        match character {
            '(' | '[' | '<' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '>' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        current.push(character);
    }
    parts.push(current);
    parts
}

/// Returns the name and the parameter list of every `pub async fn` of `source`.
fn handler_parameters(source: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for chunk in source.split("pub async fn ").skip(1) {
        let (name, after) = chunk
            .split_once('(')
            .expect("a handler has a parameter list");
        let mut depth = 1_usize;
        let mut parameters = String::new();
        for character in after.chars() {
            match character {
                '(' => depth = depth.saturating_add(1),
                ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if depth == 0 {
                break;
            }
            parameters.push(character);
        }
        found.push((name.trim().to_owned(), parameters));
    }
    found
}

/// Returns every handler parameter whose type is not an allowed extractor.
fn disallowed_parameters(source: &str) -> Vec<String> {
    let mut disallowed = Vec::new();
    for (name, parameters) in handler_parameters(source) {
        for parameter in split_top_level(&parameters, ",") {
            let parameter = parameter.trim();
            if parameter.is_empty() {
                continue;
            }
            let parts = split_top_level(parameter, ": ");
            let kind = parts.iter().skip(1).cloned().collect::<Vec<_>>().join(": ");
            if !ALLOWED_EXTRACTORS
                .iter()
                .any(|allowed| kind.trim().starts_with(allowed))
            {
                disallowed.push(format!("{name}: {parameter}"));
            }
        }
    }
    disallowed
}

#[test]
fn handlers_use_only_the_problem_aware_extractors() {
    let source = include_str!("../handler.rs");

    assert_eq!(disallowed_parameters(source), Vec::<String>::new());
    let names: Vec<String> = handler_parameters(source)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        names,
        ["list_event_types", "event_stream_status", "stream", "ws"]
    );
    assert!(source.contains("ValidatedQuery<ListEventTypesParams>"));
    assert!(source.contains("headers: HeaderMap,"));
    assert!(source.contains("ValidatedQuery<StreamParams>"));
    assert!(source.contains("ValidatedQuery<WsParams>"));
    assert!(source.contains("upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,"));
}

#[rstest]
#[case::qualified_json("Json(b): axum::Json<T>")]
#[case::bare_json("Json(b): Json<T>")]
#[case::query("Query(p): Query<T>")]
#[case::path("Path(id): Path<String>")]
#[case::raw_query("RawQuery(q): RawQuery")]
#[case::raw_body("body: String")]
#[case::other_state("State(state): State<OtherState>")]
fn the_extractor_check_reports_a_planted_violation(#[case] parameter: &str) {
    let source = format!(
        "pub async fn ok(State(state): State<EventStreamState>, {parameter}) -> Result<(), Problem> {{}}"
    );

    assert_eq!(disallowed_parameters(&source), [format!("ok: {parameter}")]);
}

#[test]
fn the_extractor_check_accepts_the_allowed_extractors() {
    let source = "pub async fn ok(State(state): State<EventStreamState>, \
        ValidatedQuery(q): ValidatedQuery<Q>) -> Result<(), Problem> {} \
        pub async fn up(State(state): State<EventStreamState>, headers: HeaderMap, \
        upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>) -> Response {}";

    assert_eq!(disallowed_parameters(source), Vec::<String>::new());
}

#[rstest]
#[case::bare_upgrade("upgrade: WebSocketUpgrade")]
#[case::other_result("upgrade: Result<WebSocketUpgrade, Problem>")]
#[case::header_extractor("TypedHeader(h): TypedHeader<LastEventId>")]
fn the_extractor_check_reports_a_fallible_streaming_extractor(#[case] parameter: &str) {
    let source = format!(
        "pub async fn ok(State(state): State<EventStreamState>, {parameter}) -> Response {{}}"
    );

    assert_eq!(disallowed_parameters(&source), [format!("ok: {parameter}")]);
}
