//! The steps of the event catalog: the configured event types and the JSON answer.

use cucumber::gherkin::Step;
use cucumber::{given, then};
use serde_json::Value;

use crate::world::AppWorld;

#[given(expr = "the event types {string} are configured")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "cucumber fixes the step signature"
)]
fn event_types_are_configured(world: &mut AppWorld, value: String) {
    world.configure_event_types(&value);
}

#[then("the JSON response should be:")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
fn json_response_should_be(world: &mut AppWorld, step: &Step) {
    let expected: Value = serde_json::from_str(
        step.docstring
            .as_deref()
            .expect("the step has a doc string"),
    )
    .expect("the doc string is JSON");
    assert_eq!(
        world.reply().json(),
        expected,
        "body: {}",
        world.reply().body
    );
}
