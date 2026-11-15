//! Seeded fixtures of the example context and of the event stream.

use event_stream_domain::EventType;
use fake::Fake as _;
use fake::faker::lorem::en::{Paragraph, Sentence};
use rand::RngExt as _;
use shared_domain::UuidGenerator as _;
use some_module_domain::{
    ExampleApiResource, ExampleApiResourceDescription, ExampleApiResourceId, ExampleApiResourceName,
};

use crate::id::SequenceUuidGenerator;
use crate::seed::TestRng;

/// A valid name: a short sentence, never longer than the limit and never containing U+0000.
///
/// # Panics
///
/// Never in practice: the generated text satisfies the invariants.
#[expect(
    clippy::expect_used,
    reason = "generated text always satisfies the invariants; a failure is a defect of the fixture"
)]
pub fn example_name(rng: &mut TestRng) -> ExampleApiResourceName {
    let text: String = Sentence(1..8).fake_with_rng(rng.rng());
    let limited = text
        .chars()
        .take(ExampleApiResourceName::MAX_CHARS)
        .collect();
    ExampleApiResourceName::new(limited).expect("a generated sentence is a valid name")
}

/// A valid description: a short paragraph, never longer than the limit and never containing U+0000.
///
/// # Panics
///
/// Never in practice: the generated text satisfies the invariants.
#[expect(
    clippy::expect_used,
    reason = "generated text always satisfies the invariants; a failure is a defect of the fixture"
)]
pub fn example_description(rng: &mut TestRng) -> ExampleApiResourceDescription {
    let text: String = Paragraph(1..3).fake_with_rng(rng.rng());
    let limited = text
        .chars()
        .take(ExampleApiResourceDescription::MAX_CHARS)
        .collect();
    ExampleApiResourceDescription::new(limited)
        .expect("a generated paragraph is a valid description")
}

/// A stored-looking aggregate (no event recorded) with a version 7 id and valid fields.
pub fn example_api_resource(rng: &mut TestRng) -> ExampleApiResource {
    ExampleApiResource::restore(
        ExampleApiResourceId::new(
            SequenceUuidGenerator::starting_at(rng.rng().random()).generate(),
        ),
        example_name(rng),
        example_description(rng),
    )
}

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

#[cfg(test)]
mod tests;
