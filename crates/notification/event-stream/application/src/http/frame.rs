//! One event encoded once for both streaming transports (FR-6, FR-9, NFR-2).

use axum::extract::ws::Utf8Bytes;
use axum::response::sse::Event;
use event_stream_domain::{EventType, Sequence, StoredEvent};

/// One event encoded once for both transports; the hub shares it between every subscriber of its
/// type.
#[derive(Debug)]
pub struct Frame {
    /// The wire id of the event.
    pub sequence: Sequence,
    /// The type of the event.
    pub event_type: EventType,
    /// The SSE event: `id` the sequence, `event` the type, the event json as its one `data` line.
    pub sse: Event,
    /// The WebSocket text: `{"id":"<sequence>","type":"<type>","data":<event json>}`.
    pub json: Utf8Bytes,
}

impl Frame {
    /// Encodes the event for both transports. `StoredEvent::json` is compact JSON, so it is one
    /// `data` line; the id is a decimal number and the type name holds only `a-z0-9._-`, so
    /// neither holds a line break.
    #[must_use]
    pub fn encode(event: &StoredEvent) -> Self {
        Self {
            sequence: event.sequence,
            event_type: event.event_type.clone(),
            sse: Event::default()
                .id(event.sequence.to_string())
                .event(event.event_type.as_str())
                .data(event.json.as_str()),
            json: Utf8Bytes::from(ws_json(event)),
        }
    }
}

/// The WebSocket frame of an event: `{"id":"` + sequence + `","type":"` + type + `","data":` +
/// json + `}`. A type name holds only `a-z0-9._-` and a sequence only digits, so neither needs
/// escaping; the json is already encoded.
pub(crate) fn ws_json(event: &StoredEvent) -> String {
    format!(
        r#"{{"id":"{}","type":"{}","data":{}}}"#,
        event.sequence,
        event.event_type.as_str(),
        event.json
    )
}

/// The WebSocket error frame `{"error":"<reason>"}`. Every reason of the protocol is a fixed
/// ASCII text without a quote, a backslash or a control character, so it needs no escaping.
#[must_use]
pub fn error_json(reason: &str) -> String {
    format!(r#"{{"error":"{reason}"}}"#)
}

#[cfg(test)]
mod tests;
