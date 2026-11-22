//! One event encoded once for every streaming transport (FR-6, FR-9, NFR-2).

use std::fmt::Write as _;
use std::sync::OnceLock;

use axum::body::Bytes;
use axum::extract::ws::Utf8Bytes;
use event_stream_domain::{EventType, Sequence, StoredEvent};
use serde::Deserialize;

/// What the WebSocket text holds before the sequence: `{"id":"`.
const WS_ID: &str = r#"{"id":""#;

/// What the WebSocket text holds between the sequence and the type.
const WS_TYPE: &str = r#"","type":""#;

/// What the WebSocket text holds between the type and the event json.
const WS_DATA: &str = r#"","data":"#;

/// What the WebSocket text holds after the event json.
const WS_END: &str = "}";

/// What the SSE block holds before the sequence.
const SSE_ID: &str = "id: ";

/// What the SSE block holds between the sequence and the type.
const SSE_EVENT: &str = "\nevent: ";

/// What the SSE block holds between the type and the event json.
const SSE_DATA: &str = "\ndata: ";

/// What the SSE block holds after the event json: the end of the `data` line and the empty line
/// that ends the block.
const SSE_END: &str = "\n\n";

/// One event encoded once for every transport; the hub shares it between every subscriber of its
/// type.
#[derive(Debug)]
pub struct Frame {
    sequence: Sequence,
    event_type: EventType,
    sse: Bytes,
    json: Utf8Bytes,
    graphql: OnceLock<async_graphql::Value>,
}

impl Frame {
    /// Encodes the event for both byte transports, each into one buffer of its exact size.
    /// `StoredEvent::json` is compact JSON, so it is one `data` line; the id is a decimal number
    /// and the type name holds only `a-z0-9._-`, so neither holds a line break.
    #[must_use]
    pub fn encode(event: &StoredEvent) -> Self {
        Self {
            sequence: event.sequence,
            event_type: event.event_type.clone(),
            sse: Bytes::from(sse_block(event)),
            json: Utf8Bytes::from(ws_json(event)),
            graphql: OnceLock::new(),
        }
    }

    /// The wire id of the event.
    #[must_use]
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }

    /// The type of the event.
    #[must_use]
    pub const fn event_type(&self) -> &EventType {
        &self.event_type
    }

    /// The finished SSE block: `id: <sequence>`, `event: <type>`, the event json as its one
    /// `data` line, then the empty line that ends the block.
    #[must_use]
    pub const fn sse(&self) -> &Bytes {
        &self.sse
    }

    /// The WebSocket text: `{"id":"<sequence>","type":"<type>","data":<event json>}`.
    #[must_use]
    pub const fn json(&self) -> &Utf8Bytes {
        &self.json
    }

    /// The wire id of the event in decimal, read from the WebSocket text, so it is written once.
    #[must_use]
    pub fn id(&self) -> &str {
        let end = WS_ID
            .len()
            .saturating_add(decimal_len(self.sequence.value()));
        self.json.as_str().get(WS_ID.len()..end).unwrap_or_default()
    }

    /// The event object as a GraphQL value, its members in the producer's order, read from the
    /// WebSocket text once by the first GraphQL subscriber and shared by every later one. A text
    /// that cannot be read gives `null`; [`Frame::encode`] never writes one, because it wraps the
    /// stored event's JSON.
    #[must_use]
    pub fn graphql_data(&self) -> &async_graphql::Value {
        self.graphql.get_or_init(|| {
            serde_json::from_str::<FrameData>(self.json.as_str())
                .map_or(async_graphql::Value::Null, |read| read.data)
        })
    }
}

/// The member `data` of a frame's WebSocket text; serde skips its members `id` and `type`.
#[derive(Debug, Deserialize)]
struct FrameData {
    /// The event object, its members in the producer's order whatever the features of
    /// `serde_json`.
    data: async_graphql::Value,
}

/// The number of decimal digits of `value`.
pub(crate) const fn decimal_len(value: u64) -> usize {
    let mut rest = value;
    let mut digits = 1_usize;
    while rest >= 10 {
        rest /= 10;
        digits = digits.saturating_add(1);
    }
    digits
}

/// An empty text whose capacity is exactly the length of `parts` plus the digits of `sequence`
/// and the type and the json of the event.
fn text_for(event: &StoredEvent, parts: &[&str]) -> String {
    let fixed = parts
        .iter()
        .map(|part| part.len())
        .fold(0_usize, usize::saturating_add);
    String::with_capacity(
        fixed
            .saturating_add(decimal_len(event.sequence.value()))
            .saturating_add(event.event_type.as_str().len())
            .saturating_add(event.json.len()),
    )
}

/// Appends the decimal digits of `sequence`, without an intermediate text; writing into a
/// `String` never fails.
fn push_sequence(text: &mut String, sequence: Sequence) {
    write!(text, "{sequence}").unwrap_or_default();
}

/// The SSE block of an event, in a text of its exact size.
pub(crate) fn sse_block(event: &StoredEvent) -> String {
    let mut text = text_for(event, &[SSE_ID, SSE_EVENT, SSE_DATA, SSE_END]);
    text.push_str(SSE_ID);
    push_sequence(&mut text, event.sequence);
    text.push_str(SSE_EVENT);
    text.push_str(event.event_type.as_str());
    text.push_str(SSE_DATA);
    text.push_str(&event.json);
    text.push_str(SSE_END);
    text
}

/// The WebSocket frame of an event, in a text of its exact size: `{"id":"` + sequence +
/// `","type":"` + type + `","data":` + json + `}`. A type name holds only `a-z0-9._-` and a
/// sequence only digits, so neither needs escaping; the json is already encoded.
pub(crate) fn ws_json(event: &StoredEvent) -> String {
    let mut text = text_for(event, &[WS_ID, WS_TYPE, WS_DATA, WS_END]);
    text.push_str(WS_ID);
    push_sequence(&mut text, event.sequence);
    text.push_str(WS_TYPE);
    text.push_str(event.event_type.as_str());
    text.push_str(WS_DATA);
    text.push_str(&event.json);
    text.push_str(WS_END);
    text
}

/// The WebSocket error frame `{"error":"<reason>"}`. Every reason of the protocol is a fixed
/// ASCII text without a quote, a backslash or a control character, so it needs no escaping.
#[must_use]
pub fn error_json(reason: &str) -> String {
    format!(r#"{{"error":"{reason}"}}"#)
}

#[cfg(test)]
mod tests;
