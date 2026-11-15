use std::any::TypeId;
use std::collections::BTreeSet;
use std::future::poll_fn;
use std::pin::pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::response::Parts;
use axum::http::{Method, Request, StatusCode};
use event_stream_domain::{
    Cursor, EventType, Offset, StoredEvent, TopicError, TopicRecord, TopicReplay,
};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde_json::{Value, json};
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

use crate::catalog::EventCatalog;
use crate::http::hub::{Hub, HubSettings};
use crate::http::subscription::{ReplaySettings, Subscriptions};
use crate::http::{EventStreamState, StreamSettings, router};
use crate::query::list_event_types::{ListEventTypes, ListEventTypesHandler};
use crate::status::StatusSource;
use crate::tail::{EventPublisher as _, TailState, TopicFault};
use crate::window::Window;

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

    assert_eq!(chunk(&mut body).await, Some(frame(1_001, "user.created")));
    assert_eq!(chunk(&mut body).await, Some(frame(1_003, "user.created")));
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
    assert_eq!(chunk(&mut body).await, Some(frame(1_001, "user.created")));
    assert_eq!(chunk(&mut body).await, Some(frame(1_002, "order.paid")));
    assert_eq!(chunk(&mut body).await, Some(frame(1_004, "user.created")));
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
    assert_eq!(chunk(&mut body).await, Some(frame(9, "user.created")));
    assert_eq!(chunk(&mut body).await, Some(frame(10, "user.created")));
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

    assert_eq!(chunk(&mut body).await, Some(frame(3, "user.created")));
    assert_eq!(chunk(&mut body).await, Some(frame(4, "user.created")));
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
            target: "event_stream_application::http::handler".to_owned(),
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

    assert_eq!(chunk(&mut body).await, Some(frame(1_001, "user.created")));
    assert_eq!(chunk(&mut body).await, Some(frame(1_002, "user.created")));
    assert_eq!(chunk(&mut body).await, None);
    assert_eq!(lines_of(&recorder, "replay failed"), Vec::<LogLine>::new());
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

    for (reply, carrier) in [(by_header, "Last-Event-ID"), (by_parameter, "lastEventId")] {
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{carrier}");
        assert_eq!(reply.content_type, PROBLEM);
        assert_eq!(
            reply.json()["detail"],
            format!("{carrier} must be a decimal unsigned 64-bit integer")
        );
    }
    assert_eq!(node.hub.subscriber_count(), 0);
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

// ---- documentation ----

fn document() -> Value {
    serde_json::to_value(router::<EventStreamState>().into_openapi()).unwrap()
}

fn problem_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": {"application/problem+json": {"schema": {"$ref": "#/components/schemas/Problem"}}}
    })
}

#[test]
fn the_document_has_the_catalog_and_the_status_operations_only() {
    let document = document();

    let paths = document["paths"].as_object().unwrap();
    let mut names: Vec<&str> = paths.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["/event-types", "/events/status"]);
    for path in names {
        assert_eq!(
            paths[path].as_object().unwrap().keys().collect::<Vec<_>>(),
            ["get"],
            "{path}"
        );
    }
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
    assert_eq!(responses["default"], problem_response("Problem response."));
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
    let responses = &operation["responses"];
    assert_eq!(
        responses["200"]["content"]["application/json"]["schema"],
        json!({"type": "array", "items": {"$ref": "#/components/schemas/EventType"}})
    );
    assert_eq!(
        responses["400"],
        problem_response("Invalid query parameter.")
    );
    assert_eq!(responses["422"], problem_response("Validation failed."));
    assert_eq!(responses["default"], problem_response("Problem response."));
    assert_eq!(
        responses.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["200", "400", "422", "default"]
    );
}

#[test]
fn the_document_carries_the_event_type_schema() {
    let document = document();

    let schemas = &document["components"]["schemas"];
    for name in ["EventStreamStatus", "EventType", "Problem"] {
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
    assert_eq!(names, ["list_event_types", "event_stream_status", "stream"]);
    assert!(source.contains("ValidatedQuery<ListEventTypesParams>"));
    assert!(source.contains("headers: HeaderMap,"));
    assert!(source.contains("ValidatedQuery<StreamParams>"));
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
