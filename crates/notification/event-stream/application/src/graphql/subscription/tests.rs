use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_graphql::{
    Context, EmptyMutation, EmptySubscription, Json, Object, Request, Response, SDLExportOptions,
    Schema,
};
use axum::extract::ws::Utf8Bytes;
use axum::response::sse::Event;
use event_stream_domain::{EventType, Sequence};
use rstest::rstest;
use serde_json::Value;
use test_support::fixture::stored_event;

use super::EventObject;
use crate::http::frame::Frame;

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

/// A query root whose one field returns the `Event` of the frame at offset 1042.
#[derive(Debug, Default)]
struct EventQuery;

#[Object]
impl EventQuery {
    /// The event at offset 1042.
    async fn event(&self, ctx: &Context<'_>) -> async_graphql::Result<EventObject> {
        ctx.data::<Arc<Resolved>>()?
            .0
            .fetch_add(1, Ordering::SeqCst);
        let object = EventObject::from(&encoded(1043, "user.created", EVENT_JSON));
        // The `Object` macro accepts only async resolvers; the object is ready at once.
        std::future::ready(Ok(object)).await
    }
}

fn encoded(sequence: u64, event_type: &str, json: &str) -> Frame {
    Frame::encode(&stored_event(sequence, event_type, json))
}

/// A frame built by hand whose WebSocket text is `json`, as `Frame::encode` never writes it.
fn hand_built(json: &'static str) -> Frame {
    Frame {
        sequence: Sequence::new(5).unwrap(),
        event_type: EventType::new("order.paid".to_owned()).unwrap(),
        sse: Event::default(),
        json: Utf8Bytes::from_static(json),
    }
}

/// The compact JSON of the object's `data`, as a GraphQL response writes it.
fn compact(object: &EventObject) -> String {
    serde_json::to_string(&object.data).unwrap()
}

fn schema(resolved: &Arc<Resolved>) -> TestSchema {
    Schema::build(EventQuery, EmptyMutation, EmptySubscription)
        .data(Arc::clone(resolved))
        .limit_complexity(MAX_COMPLEXITY)
        .finish()
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
    tokio::time::timeout(WAIT, schema.execute(Request::new(document)))
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

// ---- EventObject::from(&Frame) ----

#[test]
fn an_encoded_frame_gives_the_id_the_type_and_the_event_object_byte_for_byte() {
    let offset = 1042;

    let object = EventObject::from(&encoded(offset + 1, "user.created", EVENT_JSON));

    assert_eq!(object.id, "1043");
    assert_eq!(object.event_type, "user.created");
    assert_eq!(compact(&object), EVENT_JSON);
}

#[rstest]
#[case::the_first_id(1, "order.paid", "1")]
#[case::a_middle_id(1_000_000, "a.b-c_d", "1000000")]
#[case::the_largest_id(u64::MAX, "user.deleted", "18446744073709551615")]
fn the_id_is_the_sequence_in_decimal_and_the_type_is_the_type_name(
    #[case] sequence: u64,
    #[case] event_type: &str,
    #[case] id: &str,
) {
    let object = EventObject::from(&encoded(sequence, event_type, "{}"));

    assert_eq!(
        object,
        EventObject {
            id: id.to_owned(),
            event_type: event_type.to_owned(),
            data: Json(async_graphql::Value::from_json(serde_json::json!({})).unwrap()),
        }
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
fn the_producers_member_order_is_kept(#[case] json: &str) {
    let object = EventObject::from(&encoded(1, "a", json));

    assert_eq!(compact(&object), json);
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
fn strings_with_escapes_and_wide_characters_keep_their_value(#[case] text: &str) {
    let json = format!(
        r#"{{"id":"x","type":"a","occurredAt":"t","data":{{"text":{text},"list":[{text}]}}}}"#
    );

    let written = compact(&EventObject::from(&encoded(1, "a", &json)));

    assert_eq!(
        serde_json::from_str::<Value>(&written).unwrap(),
        serde_json::from_str::<Value>(&json).unwrap()
    );
    assert!(!written.contains(['\n', '\r']), "{written}");
}

#[rstest]
#[case::text_written_by_serde_json(r#""a\nb \"q\" c:\\d \u0001 😀""#)]
#[case::plain_text(r#""plain""#)]
fn text_that_serde_json_wrote_is_written_back_byte_for_byte(#[case] text: &str) {
    let json = format!(r#"{{"id":"x","type":"a","occurredAt":"t","data":{{"text":{text}}}}}"#);

    let written = compact(&EventObject::from(&encoded(1, "a", &json)));

    assert_eq!(written, json);
}

#[rstest]
#[case::not_json("not json")]
#[case::empty("")]
#[case::truncated(r#"{"id":"5","type":"order.paid","data":{"x":1"#)]
#[case::without_a_data_member(r#"{"id":"5","type":"order.paid"}"#)]
#[case::a_string(r#""text""#)]
#[case::null("null")]
fn a_frame_whose_text_cannot_be_read_gives_null_data(#[case] json: &'static str) {
    let object = EventObject::from(&hand_built(json));

    assert_eq!(
        object,
        EventObject {
            id: "5".to_owned(),
            event_type: "order.paid".to_owned(),
            data: Json(async_graphql::Value::Null),
        }
    );
}

#[test]
fn only_the_data_member_of_the_text_is_read() {
    let object = EventObject::from(&hand_built(
        r#"{"id":"999","type":"other.type","data":{"b":[1,"two"],"a":{}}}"#,
    ));

    assert_eq!(object.id, "5");
    assert_eq!(object.event_type, "order.paid");
    assert_eq!(compact(&object), r#"{"b":[1,"two"],"a":{}}"#);
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
