//! An event as the node holds it.

use uuid::Uuid;

use crate::value_object::event_type::EventType;
use crate::value_object::sequence::Sequence;

/// The memory a held event costs beyond its json and its type, in bytes (FR-13).
const EVENT_OVERHEAD_BYTES: usize = 128;

/// A valid event as the node holds it. `json` is the compact event object of FR-18
/// (`{"id":..,"type":..,"occurredAt":..,"data":..}`), built once by the application from the
/// record value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    /// The wire id: the offset of the record plus 1.
    pub sequence: Sequence,
    /// The id the publisher gave the event.
    pub id: Uuid,
    /// The type of the event.
    pub event_type: EventType,
    /// The encoded event object.
    pub json: String,
}

impl StoredEvent {
    /// Memory cost of the event in the window: json bytes + type bytes + 128 (FR-13's per-event
    /// overhead).
    #[must_use]
    pub fn cost(&self) -> usize {
        self.json
            .len()
            .saturating_add(self.event_type.as_str().len())
            .saturating_add(EVENT_OVERHEAD_BYTES)
    }
}

#[cfg(test)]
mod tests;
