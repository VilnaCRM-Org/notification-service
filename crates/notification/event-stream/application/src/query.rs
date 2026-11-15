//! Queries of the event stream and the view they answer with.

pub mod list_event_types;

use event_stream_domain::EventType;

/// What a query tells about an event type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTypeView {
    /// Name of the event type.
    pub name: String,
}

impl From<EventType> for EventTypeView {
    fn from(event_type: EventType) -> Self {
        Self {
            name: event_type.as_str().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests;
