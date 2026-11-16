//! GraphQL operation and result steps.

use cucumber::gherkin::Step;
use cucumber::{then, when};
use serde_json::json;

use crate::world::AppWorld;

#[when("the GraphQL operation is sent:")]
async fn operation_is_sent(world: &mut AppWorld, step: &Step) {
    let query = step
        .docstring
        .as_deref()
        .expect("the step has a doc string");
    let body = json!({"query": query}).to_string();
    world
        .send("POST", "/api/graphql", Some("application/json"), &body)
        .await;
}

#[then("the GraphQL response should have no errors")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
fn response_has_no_errors(world: &mut AppWorld) {
    let json = world.reply().json();
    assert!(
        json.is_object() && json.get("errors").is_none_or(serde_json::Value::is_null),
        "body: {}",
        world.reply().body
    );
}
