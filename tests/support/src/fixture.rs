//! Seeded fixtures of the example context.

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

#[cfg(test)]
mod tests;
