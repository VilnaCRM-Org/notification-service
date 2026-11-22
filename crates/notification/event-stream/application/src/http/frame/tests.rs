use std::convert::Infallible;
use std::time::Duration;

use axum::extract::ws::Utf8Bytes;
use axum::response::IntoResponse as _;
use axum::response::sse::{Event, KeepAlive, Sse};
use event_stream_domain::{EventType, Sequence};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde::Serialize;
use serde_json::{Value, json};
use test_support::fixture::stored_event;

use super::{Frame, decimal_len, error_json, sse_block, ws_json};
use crate::http::keep_alive::COMMENT;

/// The WebSocket frame as `serde_json` writes it: the oracle of `ws_json`.
#[derive(Serialize)]
struct WsFrame<'a> {
    id: String,
    r#type: &'a str,
    data: &'a Value,
}

/// The WebSocket error frame as `serde_json` writes it: the oracle of `error_json`.
#[derive(Serialize)]
struct ErrorFrame<'a> {
    error: &'a str,
}

const EVENT_JSON: &str = r#"{"id":"0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"name":"x"}}"#;

/// The bytes axum's SSE response writes for this one event: the oracle of `sse_block`.
async fn axum_sse_bytes(event: Event) -> String {
    let events = futures_util::stream::iter([Ok::<_, Infallible>(event)]);
    let body = tokio::time::timeout(
        Duration::from_secs(5),
        Sse::new(events).into_response().into_body().collect(),
    )
    .await
    .expect("the SSE body completes in time")
    .unwrap()
    .to_bytes();
    String::from_utf8(body.to_vec()).unwrap()
}

/// The bytes axum's SSE response writes as its default keep-alive comment.
async fn axum_keep_alive_bytes() -> String {
    let events = futures_util::stream::pending::<Result<Event, Infallible>>();
    let mut body = Sse::new(events)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))
        .into_response()
        .into_body();
    let frame = tokio::time::timeout(Duration::from_secs(5), body.frame())
        .await
        .expect("the keep-alive comes in time")
        .unwrap()
        .unwrap();
    String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap()
}

#[tokio::test]
async fn the_sse_block_has_the_sequence_as_id_the_type_as_event_and_the_json_as_its_one_data_line()
{
    let frame = Frame::encode(&stored_event(1043, "user.created", EVENT_JSON));

    assert_eq!(
        frame.sse().as_ref(),
        format!("id: 1043\nevent: user.created\ndata: {EVENT_JSON}\n\n").as_bytes()
    );
}

#[rstest]
#[case::the_first_id(1, "a")]
#[case::a_middle_id(1_043, "user.created")]
#[case::the_largest_id(u64::MAX, "a.b_c-d9")]
#[tokio::test(start_paused = true)]
async fn the_sse_block_is_byte_equal_to_the_event_axum_writes(
    #[case] sequence: u64,
    #[case] event_type: &str,
) {
    let event = stored_event(sequence, event_type, EVENT_JSON);

    let oracle = axum_sse_bytes(
        Event::default()
            .id(sequence.to_string())
            .event(event_type)
            .data(EVENT_JSON),
    )
    .await;

    assert_eq!(sse_block(&event), oracle);
    assert_eq!(Frame::encode(&event).sse().as_ref(), oracle.as_bytes());
}

#[tokio::test(start_paused = true)]
async fn the_keep_alive_comment_is_byte_equal_to_the_default_comment_of_axum() {
    assert_eq!(COMMENT, axum_keep_alive_bytes().await);
}

#[test]
fn the_websocket_text_wraps_the_json_with_the_sequence_as_a_string_and_the_type() {
    let frame = Frame::encode(&stored_event(1043, "user.created", EVENT_JSON));

    assert_eq!(
        frame.json().as_str(),
        format!(r#"{{"id":"1043","type":"user.created","data":{EVENT_JSON}}}"#)
    );
}

#[test]
fn the_frame_carries_the_sequence_and_the_type_of_its_event() {
    let frame = Frame::encode(&stored_event(7, "order.paid", "{}"));

    assert_eq!(frame.sequence(), Sequence::new(7).unwrap());
    assert_eq!(
        frame.event_type(),
        &EventType::new("order.paid".to_owned()).unwrap()
    );
}

#[rstest]
#[case::zero(0, 1)]
#[case::one_digit(9, 1)]
#[case::two_digits(10, 2)]
#[case::still_two_digits(99, 2)]
#[case::three_digits(100, 3)]
#[case::nineteen_digits(9_999_999_999_999_999_999, 19)]
#[case::twenty_digits(10_000_000_000_000_000_000, 20)]
#[case::the_largest(u64::MAX, 20)]
fn decimal_len_counts_the_digits(#[case] value: u64, #[case] digits: usize) {
    assert_eq!(decimal_len(value), digits);
    assert_eq!(decimal_len(value), value.to_string().len());
}

#[rstest]
#[case::one_digit(1)]
#[case::four_digits(1_043)]
#[case::twenty_digits(u64::MAX)]
fn both_encodings_are_built_in_a_buffer_of_their_exact_size(#[case] sequence: u64) {
    let event = stored_event(sequence, "user.created", EVENT_JSON);

    let sse = sse_block(&event);
    let ws = ws_json(&event);

    assert_eq!(sse.capacity(), sse.len());
    assert_eq!(ws.capacity(), ws.len());
}

#[rstest]
#[case::one_digit(1, "1")]
#[case::four_digits(1_043, "1043")]
#[case::twenty_digits(u64::MAX, "18446744073709551615")]
fn the_id_is_the_sequence_in_decimal(#[case] sequence: u64, #[case] id: &str) {
    let frame = Frame::encode(&stored_event(sequence, "user.created", EVENT_JSON));

    assert_eq!(frame.id(), id);
}

/// A frame built by hand whose WebSocket text is `json`, as `Frame::encode` never writes it.
fn hand_built(json: &'static str) -> Frame {
    Frame {
        sequence: Sequence::new(5).unwrap(),
        event_type: EventType::new("order.paid".to_owned()).unwrap(),
        sse: axum::body::Bytes::new(),
        json: Utf8Bytes::from_static(json),
        graphql: std::sync::OnceLock::new(),
    }
}

#[test]
fn the_graphql_data_is_the_event_object_in_the_producers_member_order() {
    let json =
        r#"{"type":"a","id":"x","data":{"z":{"y":[{"b":1,"a":2}]},"a":"last"},"occurredAt":"t"}"#;
    let frame = Frame::encode(&stored_event(1, "a", json));

    assert_eq!(serde_json::to_string(frame.graphql_data()).unwrap(), json);
}

#[test]
fn the_graphql_data_is_read_once_and_shared() {
    let frame = Frame::encode(&stored_event(1, "a", EVENT_JSON));

    assert!(std::ptr::eq(frame.graphql_data(), frame.graphql_data()));
}

#[test]
fn only_the_data_member_of_the_text_is_read() {
    let frame = hand_built(r#"{"id":"999","type":"other.type","data":{"b":[1,"two"],"a":{}}}"#);

    assert_eq!(
        serde_json::to_string(frame.graphql_data()).unwrap(),
        r#"{"b":[1,"two"],"a":{}}"#
    );
}

#[rstest]
#[case::not_json("not json")]
#[case::empty("")]
#[case::truncated(r#"{"id":"5","type":"order.paid","data":{"x":1"#)]
#[case::without_a_data_member(r#"{"id":"5","type":"order.paid"}"#)]
#[case::a_string(r#""text""#)]
#[case::null("null")]
fn a_frame_whose_text_cannot_be_read_gives_null_graphql_data(#[case] json: &'static str) {
    let frame = hand_built(json);

    assert_eq!(frame.graphql_data(), &async_graphql::Value::Null);
}

#[rstest]
#[case::quotes(json!({"text": "say \"hi\""}))]
#[case::backslashes(json!({"path": "c:\\dir\\file"}))]
#[case::control_characters(json!({"text": "a\u{0}b\u{1}c\td\ne\rf\u{1f}"}))]
#[case::outside_the_basic_multilingual_plane(json!({"text": "\u{1F600}\u{10FFFF}\u{20000}"}))]
#[case::nested(json!({"list": [1, -2.5, true, null, {"inner": "\u{7f}\u{2028}"}]}))]
fn ws_json_is_byte_equal_to_serde_json(#[case] data: Value) {
    let event = stored_event(18_446_744_073_709_551_615, "a.b_c-d9", &data.to_string());

    let expected = serde_json::to_string(&WsFrame {
        id: event.sequence.to_string(),
        r#type: event.event_type.as_str(),
        data: &data,
    })
    .unwrap();

    assert_eq!(ws_json(&event), expected);
}

#[rstest]
#[case::invalid_command("invalid command")]
#[case::invalid_name("invalid event type name")]
#[case::types_bound("types must hold 1 to 100 event type names")]
#[case::too_many_types("a connection subscribes to at most 100 event types")]
#[case::binary("binary frames are not supported")]
#[case::too_large("the command is too large")]
fn error_json_is_byte_equal_to_serde_json_for_every_reason_of_the_protocol(#[case] reason: &str) {
    let expected = serde_json::to_string(&ErrorFrame { error: reason }).unwrap();

    assert_eq!(error_json(reason), expected);
}
