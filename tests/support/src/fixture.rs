//! Seeded fixtures of the event stream.

use event_stream_domain::{EventType, Sequence, StoredEvent};
use rand::RngExt as _;

use crate::seed::TestRng;

/// The first character of an event type name.
const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";

/// The characters of a run of an event type name after its first letter.
const RUN: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// The separators between the runs of an event type name.
const SEPARATORS: &[u8] = b"._-";

/// One random byte of `alphabet`, which is never empty.
fn pick(rng: &mut TestRng, alphabet: &[u8]) -> char {
    let index = rng.rng().random_range(0..alphabet.len());
    char::from(alphabet.get(index).copied().unwrap_or(b'a'))
}

/// A valid event type name: a letter, up to 8 more letters or digits, then up to 4 runs of 1 to 8
/// letters or digits, each after one separator; at most 45 characters.
///
/// # Panics
///
/// Never in practice: the generated name follows the grammar of event type names.
#[expect(
    clippy::expect_used,
    reason = "the generated name follows the grammar; a failure is a defect of the fixture"
)]
pub fn event_type(rng: &mut TestRng) -> EventType {
    let mut name = String::new();
    name.push(pick(rng, LETTERS));
    for _ in 0..rng.rng().random_range(0..=8) {
        name.push(pick(rng, RUN));
    }
    for _ in 0..rng.rng().random_range(0..=4) {
        name.push(pick(rng, SEPARATORS));
        for _ in 0..rng.rng().random_range(1..=8) {
            name.push(pick(rng, RUN));
        }
    }
    EventType::new(name).expect("a generated name follows the grammar")
}

/// A held event with the wire id `sequence`, the type `event_type` and the encoded object `json`;
/// its publisher id is the sequence as a UUID, so two events of different ids never share one.
///
/// # Panics
///
/// When `sequence` is 0 or `event_type` is not a valid event type name: a defect of the test.
#[expect(
    clippy::expect_used,
    reason = "the arguments are literals of the calling test; a failure is a defect of that test"
)]
#[must_use]
pub fn stored_event(sequence: u64, event_type: &str, json: &str) -> StoredEvent {
    StoredEvent {
        sequence: Sequence::new(sequence).expect("a test sequence is 1 or greater"),
        id: uuid::Uuid::from_u128(u128::from(sequence)),
        event_type: EventType::new(event_type.to_owned()).expect("a valid test event type"),
        json: json.to_owned(),
    }
}

#[cfg(test)]
mod tests;
