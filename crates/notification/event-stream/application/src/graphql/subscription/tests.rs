use std::fmt::Write as _;
use std::future::{Future, poll_fn};
use std::pin::{Pin, pin};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Poll;
use std::time::Duration;

use async_graphql::{
    Context, EmptyMutation, EmptySubscription, Json, Object, OutputType, Request, Response,
    SDLExportOptions, Schema, Variables,
};
use axum::http::HeaderValue;
use event_stream_domain::{Cursor, Offset, Sequence, TopicError, TopicRecord, TopicReplay};
use futures_util::StreamExt as _;
use futures_util::stream::BoxStream;
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

use super::{EventData, EventObject, EventStreamSubscription, StreamLink, StreamPhase};
use crate::graphql::EventStreamQuery;
use crate::http::frame::Frame;
use crate::http::hub::{Delivery, Hub, HubError, HubSettings};
use crate::http::subscription::Subscriptions;
use crate::http::test_node::{Node, POSITION, REPLAY, event_json};
use crate::http::{EventStreamState, StreamSettings};
use crate::status::StatusSource;
use crate::tail::TailState;
use crate::window::Window;

/// The bound of every awaited document of these tests.
const WAIT: Duration = Duration::from_secs(5);

/// The complexity limit of the composition root's schema (`MAX_COMPLEXITY`), which this crate
/// cannot name; the schema tests of the composition root pin the same relation.
const MAX_COMPLEXITY: usize = 1_000;

/// An event object as the producer wrote it: the envelope members are not in sorted order.
const EVENT_JSON: &str = r#"{"id":"0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"name":"x"}}"#;

/// The `Event` type and the `JSON` scalar as the sorted SDL of FR-1 prints them.
const EVENT_SDL: &str = r#""An event of a subscribed type: the same object as a WebSocket frame."
type Event {
  "The event object the producer wrote: envelope id, type, occurredAt and data."
  data: JSON!
  "Event id: the Kafka offset of the event plus one, in decimal; the cursor of a resume."
  id: String!
  "Event type name."
  type: String!
}
"#;

const JSON_SDL: &str = "\"A scalar that can represent any JSON value.\"\nscalar JSON\n";

type TestSchema = Schema<EventQuery, EmptyMutation, EmptySubscription>;

/// The number of times the query root resolved its `event` field.
#[derive(Debug, Default)]
struct Resolved(AtomicUsize);

/// A query root whose one field returns the `Event` of the frame in the schema data.
#[derive(Debug, Default)]
struct EventQuery;

#[Object]
impl EventQuery {
    /// The event of the frame in the schema data.
    async fn event(&self, ctx: &Context<'_>) -> async_graphql::Result<EventObject> {
        ctx.data::<Arc<Resolved>>()?
            .0
            .fetch_add(1, Ordering::SeqCst);
        let object = EventObject::from(Arc::clone(ctx.data::<Arc<Frame>>()?));
        // The `Object` macro accepts only async resolvers; the object is ready at once.
        std::future::ready(Ok(object)).await
    }
}

fn encoded(sequence: u64, event_type: &str, json: &str) -> Frame {
    Frame::encode(&stored_event(sequence, event_type, json))
}

/// A schema over the frame of the event at offset 1042.
fn schema(resolved: &Arc<Resolved>) -> TestSchema {
    schema_of(resolved, encoded(1043, "user.created", EVENT_JSON))
}

fn schema_of(resolved: &Arc<Resolved>, frame: Frame) -> TestSchema {
    Schema::build(EventQuery, EmptyMutation, EmptySubscription)
        .data(Arc::clone(resolved))
        .data(Arc::new(frame))
        .limit_complexity(MAX_COMPLEXITY)
        .finish()
}

/// The compact JSON of the `event` field of `document` over `frame`, as a GraphQL response writes
/// it.
async fn written(frame: Frame, document: &str) -> String {
    let response = execute(&schema_of(&Arc::default(), frame), document).await;
    assert_eq!(messages(&response), Vec::<&str>::new());
    let text = serde_json::to_string(&response.data).unwrap();
    text.strip_prefix(r#"{"event":"#)
        .and_then(|rest| rest.strip_suffix('}'))
        .unwrap()
        .to_owned()
}

/// The compact JSON of the `data` of the `Event` of `frame`.
async fn compact(frame: Frame) -> String {
    let event = written(frame, "{ event { data } }").await;
    event
        .strip_prefix(r#"{"data":"#)
        .and_then(|rest| rest.strip_suffix('}'))
        .unwrap()
        .to_owned()
}

fn sdl() -> String {
    schema(&Arc::default()).sdl_with_options(
        SDLExportOptions::new()
            .sorted_fields()
            .sorted_arguments()
            .sorted_enum_items()
            .prefer_single_line_descriptions()
            .use_space_ident()
            .indent_width(2),
    )
}

async fn execute(schema: &TestSchema, document: &str) -> Response {
    timeout(WAIT, schema.execute(Request::new(document)))
        .await
        .expect("the document answers in time")
}

fn messages(response: &Response) -> Vec<&str> {
    response
        .errors
        .iter()
        .map(|error| error.message.as_str())
        .collect()
}

// ---- the Event of a Frame ----

#[tokio::test]
async fn an_encoded_frame_gives_the_id_the_type_and_the_event_object_byte_for_byte() {
    let offset = 1042;

    let event = written(
        encoded(offset + 1, "user.created", EVENT_JSON),
        "{ event { id type data } }",
    )
    .await;

    assert_eq!(
        event,
        format!(r#"{{"id":"1043","type":"user.created","data":{EVENT_JSON}}}"#)
    );
}

#[rstest]
#[case::the_first_id(1, "order.paid", "1")]
#[case::a_middle_id(1_000_000, "a.b-c_d", "1000000")]
#[case::the_largest_id(u64::MAX, "user.deleted", "18446744073709551615")]
#[tokio::test]
async fn the_id_is_the_sequence_in_decimal_and_the_type_is_the_type_name(
    #[case] sequence: u64,
    #[case] event_type: &str,
    #[case] id: &str,
) {
    let event = written(
        encoded(sequence, event_type, "{}"),
        "{ event { type data id } }",
    )
    .await;

    assert_eq!(
        event,
        format!(r#"{{"type":"{event_type}","data":{{}},"id":"{id}"}}"#)
    );
}

#[rstest]
#[case::inside_data(
    r#"{"id":"0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"z":1,"a":2}}"#
)]
#[case::nested_objects_and_arrays(
    r#"{"type":"a","id":"x","data":{"z":{"y":[{"b":1,"a":2},{"d":true,"c":null}]},"a":"last"},"occurredAt":"t"}"#
)]
#[case::numbers(r#"{"z":-1,"y":0,"x":18446744073709551615,"w":-9223372036854775808,"v":1.5}"#)]
#[tokio::test]
async fn the_producers_member_order_is_kept(#[case] json: &str) {
    assert_eq!(compact(encoded(1, "a", json)).await, json);
}

#[rstest]
#[case::line_feed(r#""a\nb""#)]
#[case::carriage_return_and_line_feed(r#""a\r\nb\rc""#)]
#[case::unicode_line_separators("\"a\u{2028}b\u{2029}c\"")]
#[case::quotes(r#""say \"hi\"""#)]
#[case::backslashes(r#""c:\\dir\\file""#)]
#[case::control_characters(r#""a\u0000b\u001fc\td\be\f""#)]
#[case::outside_the_basic_multilingual_plane("\"\u{1F600} \u{1D11E} \u{10FFFF}\"")]
#[case::escaped_surrogate_pair(r#""\ud83d\ude00""#)]
#[case::escaped_characters_of_the_plane(r#""\u00e9\u20ac""#)]
#[tokio::test]
async fn strings_with_escapes_and_wide_characters_keep_their_value(#[case] text: &str) {
    let json = format!(
        r#"{{"id":"x","type":"a","occurredAt":"t","data":{{"text":{text},"list":[{text}]}}}}"#
    );

    let written = compact(encoded(1, "a", &json)).await;

    assert_eq!(
        serde_json::from_str::<Value>(&written).unwrap(),
        serde_json::from_str::<Value>(&json).unwrap()
    );
    assert!(!written.contains(['\n', '\r']), "{written}");
}

#[rstest]
#[case::text_written_by_serde_json(r#""a\nb \"q\" c:\\d \u0001 😀""#)]
#[case::plain_text(r#""plain""#)]
#[tokio::test]
async fn text_that_serde_json_wrote_is_written_back_byte_for_byte(#[case] text: &str) {
    let json = format!(r#"{{"id":"x","type":"a","occurredAt":"t","data":{{"text":{text}}}}}"#);

    assert_eq!(compact(encoded(1, "a", &json)).await, json);
}

#[test]
fn the_event_data_is_the_json_scalar_of_async_graphql() {
    assert_eq!(
        <EventData<'_> as OutputType>::type_name(),
        <Json<async_graphql::Value> as OutputType>::type_name()
    );
    assert_eq!(
        <EventData<'_> as OutputType>::qualified_type_name(),
        "JSON!"
    );
}

#[tokio::test]
async fn every_object_of_a_frame_resolves_the_one_shared_value() {
    let frame = Arc::new(encoded(1, "a", EVENT_JSON));
    let read = frame.graphql_data();

    let first = EventObject::from(Arc::clone(&frame));
    let second = EventObject::from(Arc::clone(&frame));

    assert!(std::ptr::eq(first.frame.graphql_data(), read));
    assert!(std::ptr::eq(second.frame.graphql_data(), read));
}

// ---- the Event type in a schema ----

#[test]
fn the_sdl_has_the_event_type_with_its_three_described_fields_and_the_json_scalar() {
    let text = sdl();

    assert!(text.contains(EVENT_SDL), "{text}");
    assert!(text.contains(JSON_SDL), "{text}");
}

#[test]
fn no_description_of_the_event_type_holds_a_forbidden_word() {
    let text = sdl();

    let descriptions: Vec<&str> = EVENT_SDL
        .lines()
        .filter(|line| line.trim_start().starts_with('"'))
        .collect();
    assert_eq!(descriptions.len(), 4);
    for description in descriptions {
        assert!(text.contains(description), "{text}");
        for forbidden in ["node", "Connection", "update", "delete", "input "] {
            assert!(
                !description.contains(forbidden),
                "`{description}` contains `{forbidden}`"
            );
        }
    }
}

#[tokio::test]
async fn a_document_that_selects_data_once_runs_and_keeps_the_member_order() {
    let resolved = Arc::new(Resolved::default());

    let response = execute(&schema(&resolved), "{ event { id type data } }").await;

    assert_eq!(messages(&response), Vec::<&str>::new());
    assert_eq!(
        serde_json::to_string(&response.data).unwrap(),
        format!(r#"{{"event":{{"id":"1043","type":"user.created","data":{EVENT_JSON}}}}}"#)
    );
    assert_eq!(resolved.0.load(Ordering::SeqCst), 1);
}

#[rstest]
#[case::under_two_aliases("{ event { a: data b: data } }")]
#[case::one_name_written_twice("{ event { data data } }")]
#[case::through_a_fragment_spread("{ event { data ...F } } fragment F on Event { data }")]
#[case::through_an_inline_fragment("{ event { data ... on Event { data } } }")]
#[tokio::test]
async fn a_document_that_selects_data_twice_is_refused_before_any_field_is_resolved(
    #[case] document: &str,
) {
    let resolved = Arc::new(Resolved::default());

    let response = execute(&schema(&resolved), document).await;

    assert_eq!(messages(&response), ["Query is too complex."]);
    assert_eq!(response.data, async_graphql::Value::Null);
    assert_eq!(resolved.0.load(Ordering::SeqCst), 0);
}

/// `event` costs 1 plus its children, `data` 501 and each `id` 1: 498 aliased ids reach the limit
/// of 1 000 exactly, one more exceeds it.
#[rstest]
#[case::at_the_limit(498, Some("i497"))]
#[case::one_above_the_limit(499, None)]
#[tokio::test]
async fn data_costs_501_of_the_complexity_limit(#[case] ids: usize, #[case] last: Option<&str>) {
    let resolved = Arc::new(Resolved::default());
    let aliases = (0..ids).fold(String::new(), |mut text, n| {
        write!(text, " i{n}: id").unwrap();
        text
    });

    let response = execute(
        &schema(&resolved),
        &format!("{{ event {{ data{aliases} }} }}"),
    )
    .await;

    if let Some(last) = last {
        assert_eq!(messages(&response), Vec::<&str>::new());
        assert_eq!(resolved.0.load(Ordering::SeqCst), 1);
        let data = response.data.into_json().unwrap();
        assert_eq!(data["event"]["i0"], "1043");
        assert_eq!(data["event"][last], "1043");
        assert_eq!(
            data["event"]["data"],
            serde_json::from_str::<Value>(EVENT_JSON).unwrap()
        );
    } else {
        assert_eq!(messages(&response), ["Query is too complex."]);
        assert_eq!(response.data, async_graphql::Value::Null);
        assert_eq!(resolved.0.load(Ordering::SeqCst), 0);
    }
}

/// The subscription root and its `events` field as the sorted SDL of FR-1 prints them. The root
/// carries no description of its own here: the merged root of the composition root names it.
const SUBSCRIPTION_SDL: &str = r#"
type EventStreamSubscription {
  "Streams every event of the listed types (1 to 100 names); with lastEventId or the Last-Event-ID header, every retained event after that id comes first."
  events(lastEventId: String, types: [String!]!): Event!
}
"#;

#[test]
fn the_sdl_has_the_events_field_with_its_one_line_description_and_its_signature() {
    let text = Schema::build(EventStreamQuery, EmptyMutation, EventStreamSubscription)
        .finish()
        .sdl_with_options(
            SDLExportOptions::new()
                .sorted_fields()
                .sorted_arguments()
                .prefer_single_line_descriptions()
                .use_space_ident()
                .indent_width(2),
        );

    assert!(text.contains(SUBSCRIPTION_SDL), "{text}");
    assert!(text.contains(EVENT_SDL), "{text}");
    for forbidden in ["node", "Connection", "update", "delete", "input "] {
        assert!(!SUBSCRIPTION_SDL.contains(forbidden), "{forbidden}");
    }
}

// ---- StreamLink ----

/// Polls `future` once with the context of the test task: true when it is still pending.
async fn still_pending<F: Future>(mut future: Pin<&mut F>) -> bool {
    poll_fn(|context| Poll::Ready(future.as_mut().poll(context).is_pending())).await
}

fn sequence(id: u64) -> Sequence {
    Sequence::new(id).unwrap()
}

/// Every phase a link can be put in, with the step that puts a new link there.
fn link_in(phase: StreamPhase) -> StreamLink {
    let link = StreamLink::new(None);
    match phase {
        StreamPhase::Pending => {}
        StreamPhase::Opened => link.opened(),
        StreamPhase::Refused(error) => link.refused(error),
        StreamPhase::Released => {
            link.opened();
            link.release();
        }
    }
    link
}

#[rstest]
#[case::with_a_header(Some("42"))]
#[case::without_a_header(None)]
fn a_new_link_is_pending_with_the_header_given_and_nothing_delivered(#[case] header: Option<&str>) {
    let value = header.map(|text| HeaderValue::from_str(text).unwrap());

    let link = StreamLink::new(value.clone());

    assert_eq!(link.phase(), StreamPhase::Pending);
    assert_eq!(link.last_event_id(), value.as_ref());
    assert_eq!(link.take_delivered(), None);
}

#[tokio::test(start_paused = true)]
async fn decided_pends_while_pending_and_wakes_when_the_field_opens() {
    let link = StreamLink::new(None);
    let mut decided = pin!(link.decided());
    assert!(still_pending(decided.as_mut()).await);
    assert!(timeout(WAIT, link.decided()).await.is_err());

    link.opened();

    assert_eq!(link.phase(), StreamPhase::Opened);
    assert!(timeout(WAIT, decided).await.is_ok());
    assert!(timeout(WAIT, link.decided()).await.is_ok());
}

#[rstest]
#[case::closed(HubError::Closed)]
#[case::not_ready(HubError::NotReady)]
#[case::at_capacity(HubError::AtCapacity)]
#[tokio::test(start_paused = true)]
async fn decided_wakes_when_the_hub_refuses(#[case] error: HubError) {
    let link = StreamLink::new(None);
    let mut decided = pin!(link.decided());
    assert!(still_pending(decided.as_mut()).await);

    link.refused(error);

    assert_eq!(link.phase(), StreamPhase::Refused(error));
    assert!(timeout(WAIT, decided).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn release_turns_opened_into_released_and_wakes_released() {
    let link = link_in(StreamPhase::Opened);
    let mut released = pin!(link.released());
    assert!(still_pending(released.as_mut()).await);
    assert!(timeout(WAIT, link.released()).await.is_err());

    link.release();

    assert_eq!(link.phase(), StreamPhase::Released);
    assert!(timeout(WAIT, released).await.is_ok());
    assert!(timeout(WAIT, link.released()).await.is_ok());
    assert!(timeout(WAIT, link.decided()).await.is_ok());
}

#[rstest]
#[case::pending(StreamPhase::Pending)]
#[case::refused_closed(StreamPhase::Refused(HubError::Closed))]
#[case::refused_not_ready(StreamPhase::Refused(HubError::NotReady))]
#[case::refused_at_capacity(StreamPhase::Refused(HubError::AtCapacity))]
#[tokio::test(start_paused = true)]
async fn release_changes_nothing_unless_the_field_opened(#[case] phase: StreamPhase) {
    let link = link_in(phase);

    link.release();

    assert_eq!(link.phase(), phase);
    assert!(timeout(WAIT, link.released()).await.is_err());
}

#[tokio::test(start_paused = true)]
async fn a_second_release_keeps_the_link_released() {
    let link = link_in(StreamPhase::Released);

    link.release();

    assert_eq!(link.phase(), StreamPhase::Released);
    assert!(timeout(WAIT, link.released()).await.is_ok());
}

#[test]
fn take_delivered_answers_the_last_delivered_id_once() {
    let link = StreamLink::new(None);

    link.delivered(sequence(7));
    assert_eq!(link.take_delivered(), Some(sequence(7)));
    assert_eq!(link.take_delivered(), None);

    link.delivered(sequence(8));
    link.delivered(sequence(u64::MAX));
    assert_eq!(link.take_delivered(), Some(sequence(u64::MAX)));
    assert_eq!(link.take_delivered(), None);
}

#[test]
fn every_clone_of_a_link_shares_its_phase_and_its_delivered_id() {
    let link = StreamLink::new(Some(HeaderValue::from_static("9")));
    let transport = link.clone();

    link.opened();
    transport.release();
    link.delivered(sequence(3));

    assert_eq!(link.phase(), StreamPhase::Released);
    assert_eq!(transport.phase(), StreamPhase::Released);
    assert_eq!(transport.take_delivered(), Some(sequence(3)));
    assert_eq!(link.take_delivered(), None);
    assert_eq!(
        transport.last_event_id(),
        Some(&HeaderValue::from_static("9"))
    );
}

// ---- the events field ----

type StreamSchema = Schema<EventStreamQuery, EmptyMutation, EventStreamSubscription>;

type Responses = BoxStream<'static, Response>;

/// The hub of the field tests: eight subscribers, a queue of 64 frames.
const HUB: HubSettings = HubSettings {
    max_subscribers: 8,
    buffer: 64,
};

/// The window of the field tests, in events.
const WINDOW: usize = 100;

/// A subscription of `a` that selects every field of `Event`.
const EVENTS: &str = r#"subscription { events(types: ["a"]) { id type data } }"#;

/// A subscription whose arguments are the variables `types` and `lastEventId`.
const WITH_VARIABLES: &str = "subscription($types: [String!]!, $lastEventId: String) { events(types: $types, lastEventId: $lastEventId) { id } }";

const HEADER_MESSAGE: &str = "Last-Event-ID must be a decimal unsigned 64-bit integer";
const PARAMETER_MESSAGE: &str = "lastEventId must be a decimal unsigned 64-bit integer";

/// The state of a schema over the hub, the window and the tail of a node, with replays that read
/// `replay`.
fn state_of(
    hub: &Arc<Hub>,
    window: &Arc<Window>,
    tail: &watch::Sender<TailState>,
    replay: Arc<dyn TopicReplay>,
) -> EventStreamState {
    let subscriptions = Subscriptions::new(
        Arc::clone(hub),
        Arc::clone(window),
        replay,
        tail.subscribe(),
        REPLAY,
    );
    EventStreamState {
        query_bus: Arc::new(FakeQueryBus::unregistered()),
        subscriptions: Arc::new(subscriptions),
        status: Arc::new(StatusSource::new(
            tail.subscribe(),
            Arc::clone(window),
            Arc::<Hub>::clone(hub),
        )),
        hub: Arc::clone(hub),
        settings: StreamSettings {
            keep_alive: Duration::from_secs(15),
        },
    }
}

fn stream_schema(state: EventStreamState) -> StreamSchema {
    Schema::build(EventStreamQuery, EmptyMutation, EventStreamSubscription)
        .data(state)
        .finish()
}

/// A started test node (hub position and window floor [`POSITION`]) and the test schema over it.
struct Fixture {
    node: Node,
    schema: StreamSchema,
}

impl Fixture {
    /// The node with `hub` whose replays read `replay`.
    fn new(replay: Arc<dyn TopicReplay>, hub: HubSettings) -> Self {
        let node = Node::started(POSITION, Arc::clone(&replay), REPLAY, hub, WINDOW);
        let state = state_of(&node.hub, &node.window, &node.tail, replay);
        Self {
            node,
            schema: stream_schema(state),
        }
    }

    /// The node of [`HUB`] whose replays read an empty topic.
    fn ready() -> Self {
        Self::new(Arc::new(StaticTopic::empty()), HUB)
    }

    /// Runs `request` with `link` in its data.
    fn run(&self, request: Request, link: &StreamLink) -> Responses {
        self.schema.execute_stream(request.data(link.clone()))
    }

    /// Runs `document` with `link` in its data.
    fn subscribe(&self, document: &str, link: &StreamLink) -> Responses {
        self.run(Request::new(document), link)
    }

    /// What the tail does with a valid event: the window first, then the hub.
    fn pass(&self, id: u64, event_type: &str) -> Option<Delivery> {
        let event = stored_event(id, event_type, &event_json(id));
        self.node.window.push(Arc::new(event.clone()));
        self.node.hub.deliver(&event)
    }

    fn subscribers(&self) -> usize {
        self.node.hub.subscriber_count()
    }
}

/// The next response within [`WAIT`]; `None` when the stream ended.
async fn next(responses: &mut Responses) -> Option<Value> {
    timeout(WAIT, responses.next())
        .await
        .expect("the stream answers in time")
        .map(|response| serde_json::to_value(response).unwrap())
}

/// True when the stream yields nothing while [`WAIT`] passes; the clock is paused, so nothing
/// really waits.
async fn quiet(responses: &mut Responses) -> bool {
    timeout(WAIT, responses.next()).await.is_err()
}

/// The result of the event `id` of a node, with every field selected.
fn result(id: u64, event_type: &str) -> Value {
    json!({"data": {"events": {
        "id": id.to_string(),
        "type": event_type,
        "data": serde_json::from_str::<Value>(&event_json(id)).unwrap(),
    }}})
}

/// The 1-based column of the field `events` in `document`.
fn column(document: &str) -> usize {
    document.find("events").unwrap().saturating_add(1)
}

/// The error result of the field `events` of `document`.
fn error_result(document: &str, mut error: Value) -> Value {
    error["locations"] = json!([{"line": 1, "column": column(document)}]);
    error["path"] = json!(["events"]);
    json!({"data": null, "errors": [error]})
}

/// The `VALIDATION_FAILED` result of `document` with its one violation.
fn validation_failed(document: &str, property_path: &str, code: &str, message: &str) -> Value {
    error_result(
        document,
        json!({
            "message": "The request failed validation.",
            "extensions": {
                "code": "VALIDATION_FAILED",
                "violations": [{"propertyPath": property_path, "message": message, "code": code}],
            },
        }),
    )
}

/// The `INTERNAL_SERVER_ERROR` result of `document`.
fn internal_error(document: &str) -> Value {
    error_result(
        document,
        json!({
            "message": "Internal server error",
            "extensions": {"code": "INTERNAL_SERVER_ERROR"},
        }),
    )
}

/// `count` distinct valid names.
fn distinct(count: usize) -> Vec<String> {
    (0..count).map(|n| format!("t{n}")).collect()
}

fn with_variables(variables: Value) -> Request {
    Request::new(WITH_VARIABLES).variables(Variables::from_json(variables))
}

fn header(text: &str) -> HeaderValue {
    HeaderValue::from_str(text).unwrap()
}

fn lines_of(recorder: &LogRecorder, message: &str) -> Vec<LogLine> {
    recorder
        .lines()
        .into_iter()
        .filter(|line| line.message == message)
        .collect()
}

/// The record at the offset of the event `id` (offset `id - 1`) with a valid envelope.
fn record(id: u64, event_type: &str) -> TopicRecord {
    TopicRecord {
        offset: Offset::new(id.saturating_sub(1)),
        value: Some(envelope(id, event_type).into_bytes()),
    }
}

/// The envelope of the record of the event `id`, which is also its stored event object.
fn envelope(id: u64, event_type: &str) -> String {
    format!(
        r#"{{"id":"00000000-0000-0000-0000-{id:012x}","type":"{event_type}","occurredAt":"2026-01-01T00:00:00Z","data":{{"n":{id}}}}}"#
    )
}

/// The result of the event `id` read from Kafka, with every field selected.
fn kafka_result(id: u64, event_type: &str) -> Value {
    json!({"data": {"events": {
        "id": id.to_string(),
        "type": event_type,
        "data": serde_json::from_str::<Value>(&envelope(id, event_type)).unwrap(),
    }}})
}

#[tokio::test(start_paused = true)]
async fn a_valid_subscription_registers_one_subscriber_and_opens_the_link_when_first_polled() {
    let fixture = Fixture::ready();
    let link = StreamLink::new(None);

    let mut responses = fixture.subscribe(EVENTS, &link);
    assert_eq!(fixture.subscribers(), 0);
    assert_eq!(link.phase(), StreamPhase::Pending);
    assert!(timeout(WAIT, link.decided()).await.is_err());

    assert!(quiet(&mut responses).await);

    assert_eq!(fixture.subscribers(), 1);
    assert_eq!(link.phase(), StreamPhase::Opened);
    assert!(timeout(WAIT, link.decided()).await.is_ok());
    assert_eq!(link.take_delivered(), None);
}

#[tokio::test(start_paused = true)]
async fn before_release_nothing_is_yielded_and_after_it_the_window_resume_comes_in_order() {
    let fixture = Fixture::ready();
    fixture.pass(1_001, "a");
    fixture.pass(1_002, "a");
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(
        r#"subscription { events(types: ["a"], lastEventId: "1000") { id type data } }"#,
        &link,
    );

    assert!(quiet(&mut responses).await);
    fixture.pass(1_003, "a");
    assert!(quiet(&mut responses).await);
    assert_eq!(link.phase(), StreamPhase::Opened);

    link.release();

    assert_eq!(next(&mut responses).await, Some(result(1_001, "a")));
    assert_eq!(next(&mut responses).await, Some(result(1_002, "a")));
    assert_eq!(next(&mut responses).await, Some(result(1_003, "a")));
    assert!(quiet(&mut responses).await);
}

#[tokio::test(start_paused = true)]
async fn before_release_a_cursor_below_the_window_reads_nothing_from_kafka() {
    let replay = ScriptedTopicReplay::new();
    let fixture = Fixture::new(Arc::new(replay.clone()), HUB);
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(
        r#"subscription { events(types: ["a"], lastEventId: "5") { id } }"#,
        &link,
    );

    assert!(quiet(&mut responses).await);
    assert!(quiet(&mut responses).await);
    assert_eq!(link.phase(), StreamPhase::Opened);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());

    link.release();

    assert!(quiet(&mut responses).await);
    assert_eq!(replay.calls(), [Cursor::new(5)]);
}

#[tokio::test(start_paused = true)]
async fn before_release_no_window_page_is_taken() {
    let replay = ScriptedTopicReplay::new();
    let fixture = Fixture::new(Arc::new(replay.clone()), HUB);
    fixture.pass(1_001, "a");
    fixture.pass(1_002, "a");
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(
        r#"subscription { events(types: ["a"], lastEventId: "1000") { id } }"#,
        &link,
    );
    assert!(quiet(&mut responses).await);
    assert_eq!(link.phase(), StreamPhase::Opened);

    // A hundred events of another type push 1001 and 1002 out of the window: a window page taken
    // before the release would still hold them, and a page taken after it finds them evicted.
    for id in 1_003..=1_102 {
        fixture.pass(id, "b");
    }
    assert_eq!(fixture.node.window.start(), Cursor::new(1_002));
    assert!(quiet(&mut responses).await);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());

    link.release();

    assert!(quiet(&mut responses).await);
    assert_eq!(replay.calls(), [Cursor::new(1_000)]);
    assert_eq!(link.take_delivered(), None);
}

#[tokio::test(start_paused = true)]
async fn after_release_kafka_then_the_window_then_live_events_come_in_id_order_of_the_type_only() {
    let topic = StaticTopic::new(
        (1..=6)
            .map(|id| record(id, if id % 2 == 1 { "a" } else { "b" }))
            .collect(),
        Cursor::START,
        Cursor::new(POSITION),
    );
    let fixture = Fixture::new(Arc::new(topic), HUB);
    fixture.pass(1_001, "a");
    fixture.pass(1_002, "b");
    fixture.pass(1_003, "a");
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(
        r#"subscription { events(types: ["a"], lastEventId: "2") { id type data } }"#,
        &link,
    );
    assert!(quiet(&mut responses).await);

    link.release();

    assert_eq!(next(&mut responses).await, Some(kafka_result(3, "a")));
    assert_eq!(next(&mut responses).await, Some(kafka_result(5, "a")));
    assert_eq!(next(&mut responses).await, Some(result(1_001, "a")));
    assert_eq!(next(&mut responses).await, Some(result(1_003, "a")));
    assert!(quiet(&mut responses).await);
    fixture.pass(1_004, "b");
    fixture.pass(1_005, "a");
    assert_eq!(next(&mut responses).await, Some(result(1_005, "a")));
    assert!(quiet(&mut responses).await);
}

#[rstest]
#[case::by_the_argument(
    r#"subscription { events(types: ["a"], lastEventId: "1001") { id type data } }"#,
    None
)]
#[case::by_the_header(
    r#"subscription { events(types: ["a"]) { id type data } }"#,
    Some("1001")
)]
#[case::by_the_header_over_a_valid_argument(
    r#"subscription { events(types: ["a"], lastEventId: "1000") { id type data } }"#,
    Some("1001")
)]
#[tokio::test(start_paused = true)]
async fn a_resume_from_the_window_starts_after_the_cursor_of_the_carrier_that_wins(
    #[case] document: &str,
    #[case] last_event_id: Option<&str>,
) {
    let fixture = Fixture::ready();
    for id in 1_001..=1_004 {
        fixture.pass(id, if id % 2 == 1 { "a" } else { "b" });
    }
    let link = StreamLink::new(last_event_id.map(header));
    let mut responses = fixture.subscribe(document, &link);
    assert!(quiet(&mut responses).await);

    link.release();

    assert_eq!(next(&mut responses).await, Some(result(1_003, "a")));
    assert!(quiet(&mut responses).await);
    fixture.pass(1_005, "a");
    assert_eq!(next(&mut responses).await, Some(result(1_005, "a")));
}

#[tokio::test(start_paused = true)]
async fn without_a_cursor_exactly_the_events_published_after_registration_come_after_release() {
    let fixture = Fixture::ready();
    fixture.pass(1_001, "a");
    fixture.pass(1_002, "a");
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(EVENTS, &link);
    assert!(quiet(&mut responses).await);
    assert_eq!(link.phase(), StreamPhase::Opened);

    fixture.pass(1_003, "a");
    fixture.pass(1_004, "b");
    fixture.pass(1_005, "a");
    link.release();

    assert_eq!(next(&mut responses).await, Some(result(1_003, "a")));
    assert_eq!(next(&mut responses).await, Some(result(1_005, "a")));
    assert!(quiet(&mut responses).await);
}

#[rstest]
#[case::id_selected("{ id type }", json!({"id": "1001", "type": "a"}), json!({"id": "1003", "type": "a"}))]
#[case::id_aliased("{ ident: id }", json!({"ident": "1001"}), json!({"ident": "1003"}))]
#[case::id_not_selected("{ type }", json!({"type": "a"}), json!({"type": "a"}))]
#[case::through_a_fragment("{ ...F } fragment F on Event { id }", json!({"id": "1001"}), json!({"id": "1003"}))]
#[tokio::test(start_paused = true)]
async fn take_delivered_gives_the_id_of_each_result_whatever_the_selection(
    #[case] selection: &str,
    #[case] first: Value,
    #[case] second: Value,
) {
    let fixture = Fixture::ready();
    let (selection, fragments) = selection
        .split_once(" fragment")
        .map_or((selection, String::new()), |(set, rest)| {
            (set, format!(" fragment{rest}"))
        });
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(
        &format!(r#"subscription {{ events(types: ["a"]) {selection} }}{fragments}"#),
        &link,
    );
    assert!(quiet(&mut responses).await);
    link.release();
    fixture.pass(1_001, "a");
    fixture.pass(1_002, "b");
    fixture.pass(1_003, "a");

    assert_eq!(
        next(&mut responses).await,
        Some(json!({"data": {"events": first}}))
    );
    assert_eq!(link.take_delivered(), Some(sequence(1_001)));
    assert_eq!(link.take_delivered(), None);
    assert_eq!(
        next(&mut responses).await,
        Some(json!({"data": {"events": second}}))
    );
    assert_eq!(link.take_delivered(), Some(sequence(1_003)));
    assert_eq!(link.take_delivered(), None);
}

#[rstest]
#[case::no_name(json!({"types": []}), None, "types", "required", "types must hold 1 to 100 event type names")]
#[case::an_empty_name(json!({"types": ["a", ""]}), None, "types", "required", "types must not hold an empty event type name")]
#[case::a_name_of_129_characters(json!({"types": ["a".repeat(129)]}), None, "types", "length", "An event type must contain between 1 and 128 characters.")]
#[case::a_name_that_breaks_the_pattern(json!({"types": ["User.Created"]}), None, "types", "pattern", "An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$.")]
#[case::a_hundred_and_one_distinct_names(json!({"types": distinct(101)}), None, "types", "max_items", "types must hold 1 to 100 event type names")]
#[case::a_malformed_header(json!({"types": ["a"]}), Some("abc"), "Last-Event-ID", "format", HEADER_MESSAGE)]
#[case::a_malformed_argument(json!({"types": ["a"], "lastEventId": "abc"}), None, "lastEventId", "format", PARAMETER_MESSAGE)]
#[case::a_malformed_header_over_a_valid_argument(json!({"types": ["a"], "lastEventId": "5"}), Some("-1"), "Last-Event-ID", "format", HEADER_MESSAGE)]
#[case::a_malformed_header_over_a_malformed_argument(json!({"types": ["a"], "lastEventId": "abc"}), Some("-1"), "Last-Event-ID", "format", HEADER_MESSAGE)]
#[case::a_malformed_argument_beside_a_valid_header(json!({"types": ["a"], "lastEventId": "abc"}), Some("1043"), "lastEventId", "format", PARAMETER_MESSAGE)]
#[case::an_empty_argument_beside_a_valid_header(json!({"types": ["a"], "lastEventId": ""}), Some("1043"), "lastEventId", "format", PARAMETER_MESSAGE)]
#[case::faulty_types_and_a_faulty_cursor(json!({"types": ["User.Created"], "lastEventId": "abc"}), Some("abc"), "types", "pattern", "An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$.")]
#[case::the_first_faulty_name_in_list_order(json!({"types": ["a", "User.Created", ""]}), None, "types", "pattern", "An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$.")]
#[case::an_empty_name_before_a_pattern_fault(json!({"types": ["", "User.Created"]}), None, "types", "required", "types must not hold an empty event type name")]
#[tokio::test(start_paused = true)]
async fn invalid_arguments_yield_one_validation_error_register_nothing_and_log_nothing(
    #[case] variables: Value,
    #[case] last_event_id: Option<&str>,
    #[case] property_path: &str,
    #[case] code: &str,
    #[case] message: &str,
) {
    let (recorder, _guard) = LogRecorder::install();
    let fixture = Fixture::ready();
    let link = StreamLink::new(last_event_id.map(header));

    let mut responses = fixture.run(with_variables(variables), &link);

    let answer = next(&mut responses).await.unwrap();
    assert_eq!(
        answer,
        validation_failed(WITH_VARIABLES, property_path, code, message)
    );
    assert_eq!(next(&mut responses).await, None);
    assert_eq!(fixture.subscribers(), 0);
    assert_eq!(link.phase(), StreamPhase::Pending);
    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
    let written = answer.to_string();
    for value in ["User.Created", "abc", "-1", "t100"] {
        assert!(!written.contains(value), "{written}");
    }
}

#[rstest]
#[case::text("abc")]
#[case::negative("-1")]
#[case::plus("+1")]
#[case::leading_blank(" 1")]
#[case::empty("")]
#[case::overflow("18446744073709551616")]
#[tokio::test(start_paused = true)]
async fn each_malformed_header_names_the_header(#[case] text: &str) {
    let fixture = Fixture::ready();
    let link = StreamLink::new(Some(header(text)));

    let mut responses = fixture.run(with_variables(json!({"types": ["a"]})), &link);

    assert_eq!(
        next(&mut responses).await,
        Some(validation_failed(
            WITH_VARIABLES,
            "Last-Event-ID",
            "format",
            HEADER_MESSAGE
        ))
    );
    assert_eq!(next(&mut responses).await, None);
    assert_eq!(fixture.subscribers(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_header_that_is_not_visible_ascii_text_names_the_header() {
    let fixture = Fixture::ready();
    let link = StreamLink::new(Some(HeaderValue::from_bytes(b"\xff1").unwrap()));

    let mut responses = fixture.run(with_variables(json!({"types": ["a"]})), &link);

    assert_eq!(
        next(&mut responses).await,
        Some(validation_failed(
            WITH_VARIABLES,
            "Last-Event-ID",
            "format",
            HEADER_MESSAGE
        ))
    );
    assert_eq!(fixture.subscribers(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_hundred_distinct_names_with_repeats_open_one_subscriber() {
    let fixture = Fixture::ready();
    let mut types = distinct(100);
    types.extend(distinct(100));
    let link = StreamLink::new(None);

    let mut responses = fixture.run(
        with_variables(json!({"types": types, "lastEventId": "1000"})),
        &link,
    );

    assert!(quiet(&mut responses).await);
    assert_eq!(fixture.subscribers(), 1);
    assert_eq!(link.phase(), StreamPhase::Opened);
    link.release();
    fixture.pass(1_001, "t99");
    assert_eq!(
        next(&mut responses).await,
        Some(json!({"data": {"events": {"id": "1001"}}}))
    );
}

/// A hub that was never started, so it has no position; its window and tail go with it.
fn unstarted(settings: HubSettings, closed: bool) -> (Arc<Hub>, StreamSchema) {
    let hub = Hub::new(settings);
    if closed {
        hub.close();
    }
    let window = Arc::new(Window::new(WINDOW, usize::MAX));
    let (tail, _receiver) = watch::channel(TailState::default());
    let state = state_of(&hub, &window, &tail, Arc::new(StaticTopic::empty()));
    (hub, stream_schema(state))
}

async fn assert_refused(
    schema: &StreamSchema,
    hub: &Hub,
    error: HubError,
    subscribers: usize,
    recorder: &LogRecorder,
) {
    let link = StreamLink::new(None);

    let mut responses = schema.execute_stream(Request::new(EVENTS).data(link.clone()));

    assert_eq!(
        next(&mut responses).await,
        Some(error_result(EVENTS, json!({"message": error.to_string()})))
    );
    assert_eq!(next(&mut responses).await, None);
    assert_eq!(link.phase(), StreamPhase::Refused(error));
    assert!(timeout(WAIT, link.decided()).await.is_ok());
    assert_eq!(hub.subscriber_count(), subscribers);
    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
}

#[tokio::test(start_paused = true)]
async fn a_closed_hub_refuses_with_closed_and_nothing_is_logged() {
    let (recorder, _guard) = LogRecorder::install();
    let fixture = Fixture::ready();
    fixture.node.hub.close();

    assert_refused(
        &fixture.schema,
        &fixture.node.hub,
        HubError::Closed,
        0,
        &recorder,
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn a_hub_at_its_cap_refuses_with_at_capacity_and_keeps_its_subscribers() {
    let (recorder, _guard) = LogRecorder::install();
    let fixture = Fixture::new(
        Arc::new(StaticTopic::empty()),
        HubSettings {
            max_subscribers: 1,
            buffer: 4,
        },
    );
    let _held = fixture.node.open(&["z"], None);

    assert_refused(
        &fixture.schema,
        &fixture.node.hub,
        HubError::AtCapacity,
        1,
        &recorder,
    )
    .await;
}

#[rstest]
#[case::not_started(1, false, HubError::NotReady)]
#[case::not_started_and_without_room(0, false, HubError::NotReady)]
#[case::not_started_and_closed(0, true, HubError::Closed)]
#[tokio::test(start_paused = true)]
async fn the_refusals_keep_the_precedence_of_the_hub(
    #[case] max_subscribers: usize,
    #[case] closed: bool,
    #[case] error: HubError,
) {
    let (recorder, _guard) = LogRecorder::install();
    let (hub, schema) = unstarted(
        HubSettings {
            max_subscribers,
            buffer: 4,
        },
        closed,
    );

    assert_refused(&schema, &hub, error, 0, &recorder).await;
}

#[tokio::test(start_paused = true)]
async fn dropping_the_stream_before_release_frees_the_slot() {
    let fixture = Fixture::ready();
    let _held = fixture.node.open(&["z"], None);
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(EVENTS, &link);
    assert!(quiet(&mut responses).await);
    assert_eq!(fixture.subscribers(), 2);

    drop(responses);

    assert_eq!(fixture.subscribers(), 1);
}

#[tokio::test(start_paused = true)]
async fn dropping_the_stream_after_release_frees_the_slot() {
    let fixture = Fixture::ready();
    let _held = fixture.node.open(&["z"], None);
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(EVENTS, &link);
    assert!(quiet(&mut responses).await);
    link.release();
    fixture.pass(1_001, "a");
    assert_eq!(next(&mut responses).await, Some(result(1_001, "a")));
    assert_eq!(fixture.subscribers(), 2);

    drop(responses);

    assert_eq!(fixture.subscribers(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_closed_hub_ends_the_released_stream_after_the_queued_results_without_a_log_entry() {
    let (recorder, _guard) = LogRecorder::install();
    let fixture = Fixture::ready();
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(EVENTS, &link);
    assert!(quiet(&mut responses).await);
    link.release();
    fixture.pass(1_001, "a");
    fixture.pass(1_002, "a");

    fixture.node.hub.close();

    assert_eq!(next(&mut responses).await, Some(result(1_001, "a")));
    assert_eq!(next(&mut responses).await, Some(result(1_002, "a")));
    assert_eq!(next(&mut responses).await, None);
    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
}

#[tokio::test(start_paused = true)]
async fn a_replay_that_cannot_read_kafka_ends_the_stream_and_logs_replay_failed_once() {
    let (recorder, _guard) = LogRecorder::install();
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Err(TopicError::Unavailable("the broker is away".into())));
    let fixture = Fixture::new(Arc::new(replay.clone()), HUB);
    let link = StreamLink::new(Some(header("100")));
    let mut responses = fixture.subscribe(EVENTS, &link);
    assert!(quiet(&mut responses).await);

    link.release();

    assert_eq!(next(&mut responses).await, None);
    assert_eq!(replay.calls(), [Cursor::new(100)]);
    assert_eq!(link.take_delivered(), None);
    assert_eq!(
        recorder.lines(),
        [LogLine {
            level: Level::WARN,
            target: "event_stream_application::http::subscription".to_owned(),
            message: "replay failed".to_owned(),
            fields: vec![
                ("transport".to_owned(), "graphql-sse".to_owned()),
                (
                    "error".to_owned(),
                    "the topic is unavailable: the broker is away".to_owned()
                ),
            ],
        }]
    );
}

/// The one `request failed` entry of a wiring fault, with its error chain.
fn request_failed(error: &str) -> Vec<LogLine> {
    vec![LogLine {
        level: Level::ERROR,
        target: "shared_application::http::problem".to_owned(),
        message: "request failed".to_owned(),
        fields: vec![("error".to_owned(), error.to_owned())],
    }]
}

#[tokio::test(start_paused = true)]
async fn a_request_without_a_link_yields_an_internal_error_and_registers_nothing() {
    let (recorder, _guard) = LogRecorder::install();
    let fixture = Fixture::ready();

    let mut responses = fixture.schema.execute_stream(Request::new(EVENTS));

    assert_eq!(next(&mut responses).await, Some(internal_error(EVENTS)));
    assert_eq!(next(&mut responses).await, None);
    assert_eq!(fixture.subscribers(), 0);
    assert_eq!(
        lines_of(&recorder, "request failed"),
        request_failed("the GraphQL request has no StreamLink data")
    );
    assert_eq!(recorder.lines().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_schema_without_the_state_yields_an_internal_error_and_leaves_the_link_pending() {
    let (recorder, _guard) = LogRecorder::install();
    let schema = Schema::build(EventStreamQuery, EmptyMutation, EventStreamSubscription).finish();
    let link = StreamLink::new(None);

    let mut responses = schema.execute_stream(Request::new(EVENTS).data(link.clone()));

    assert_eq!(next(&mut responses).await, Some(internal_error(EVENTS)));
    assert_eq!(next(&mut responses).await, None);
    assert_eq!(link.phase(), StreamPhase::Pending);
    assert_eq!(
        lines_of(&recorder, "request failed"),
        request_failed("the GraphQL schema has no EventStreamState data")
    );
    assert_eq!(recorder.lines().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_released_subscriber_that_is_not_polled_falls_behind_without_delaying_the_others() {
    let fixture = Fixture::new(
        Arc::new(StaticTopic::empty()),
        HubSettings {
            max_subscribers: 8,
            buffer: 2,
        },
    );
    let mut other = fixture.node.open(&["a"], None);
    let link = StreamLink::new(None);
    let mut responses = fixture.subscribe(EVENTS, &link);
    assert!(quiet(&mut responses).await);
    link.release();

    let mut deliveries = Vec::new();
    for id in 1_001..=1_005 {
        deliveries.push(fixture.pass(id, "a"));
        let frame = timeout(WAIT, other.next())
            .await
            .expect("the other subscriber is served at once")
            .unwrap();
        assert_eq!(frame.sequence(), sequence(id));
    }

    let delivery = |delivered, behind| Some(Delivery { delivered, behind });
    assert_eq!(
        deliveries,
        [
            delivery(2, 0),
            delivery(2, 0),
            delivery(1, 1),
            delivery(1, 0),
            delivery(1, 0),
        ]
    );
    for id in 1_001..=1_005 {
        assert_eq!(next(&mut responses).await, Some(result(id, "a")));
        assert_eq!(link.take_delivered(), Some(sequence(id)));
    }
    assert!(quiet(&mut responses).await);
    assert_eq!(fixture.subscribers(), 2);
}
