use std::convert::Infallible;

use axum::response::IntoResponse as _;
use axum::response::sse::Sse;
use event_stream_domain::{EventType, Sequence};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde::Serialize;
use serde_json::{Value, json};
use test_support::fixture::stored_event;

use super::{Frame, error_json, ws_json};

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

/// The bytes the SSE response writes for exactly this one event.
async fn sse_bytes(frame: &Frame) -> String {
    let events = futures_util::stream::iter([Ok::<_, Infallible>(frame.sse.clone())]);
    let body = Sse::new(events)
        .into_response()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    String::from_utf8(body.to_vec()).unwrap()
}

#[tokio::test]
async fn the_sse_event_has_the_sequence_as_id_the_type_as_event_and_the_json_as_its_one_data_line()
{
    let frame = Frame::encode(&stored_event(1043, "user.created", EVENT_JSON));

    assert_eq!(
        sse_bytes(&frame).await,
        format!("id: 1043\nevent: user.created\ndata: {EVENT_JSON}\n\n")
    );
}

#[test]
fn the_websocket_text_wraps_the_json_with_the_sequence_as_a_string_and_the_type() {
    let frame = Frame::encode(&stored_event(1043, "user.created", EVENT_JSON));

    assert_eq!(
        frame.json.as_str(),
        format!(r#"{{"id":"1043","type":"user.created","data":{EVENT_JSON}}}"#)
    );
}

#[test]
fn the_frame_carries_the_sequence_and_the_type_of_its_event() {
    let frame = Frame::encode(&stored_event(7, "order.paid", "{}"));

    assert_eq!(frame.sequence, Sequence::new(7).unwrap());
    assert_eq!(
        frame.event_type,
        EventType::new("order.paid".to_owned()).unwrap()
    );
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
