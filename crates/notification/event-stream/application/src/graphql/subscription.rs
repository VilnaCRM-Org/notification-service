//! The GraphQL subscription of the event stream: the `Event` object, built in the subscriber's task
//! from the frame that the hub shares (NFR-1).

use async_graphql::{Json, SimpleObject};
use serde::Deserialize;

use crate::http::frame::Frame;

/// An event of a subscribed type: the same object as a WebSocket frame.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
#[graphql(name = "Event")]
pub struct EventObject {
    /// Event id: the Kafka offset of the event plus one, in decimal; the cursor of a resume.
    pub id: String,
    /// Event type name.
    #[graphql(name = "type")]
    pub event_type: String,
    /// The event object the producer wrote: envelope id, type, occurredAt and data.
    // More than half of the complexity limit 1 000: a document selects `data` at most once (FR-1).
    #[graphql(complexity = 501)]
    pub data: Json<async_graphql::Value>,
}

/// The member `data` of a frame's WebSocket text; serde skips its members `id` and `type`.
#[derive(Debug, Deserialize)]
struct FrameData {
    /// The event object, its members in the producer's order whatever the features of
    /// `serde_json`.
    data: async_graphql::Value,
}

impl From<&Frame> for EventObject {
    /// `id` from the sequence, `type` from the event type and `data` from the member `data` of the
    /// WebSocket text. A text that cannot be read gives a `null` `data`; `Frame::encode` never
    /// writes one, because it wraps the stored event's JSON.
    fn from(frame: &Frame) -> Self {
        Self {
            id: frame.sequence.to_string(),
            event_type: frame.event_type.as_str().to_owned(),
            data: Json(
                serde_json::from_str::<FrameData>(frame.json.as_str())
                    .map_or(async_graphql::Value::Null, |read| read.data),
            ),
        }
    }
}

#[cfg(test)]
mod tests;
