//! Request, status, body, header and JSON-field steps.

use cucumber::gherkin::Step;
use cucumber::{then, when};

use crate::world::AppWorld;

fn doc_string(step: &Step) -> &str {
    step.docstring
        .as_deref()
        .expect("the step has a doc string")
}

#[when(expr = "{word} request is sent to {string}")]
async fn request_is_sent(world: &mut AppWorld, method: String, path: String) {
    world.send(&method, &path, None, "").await;
}

#[when(expr = "{word} request is sent to {string} with body:")]
async fn request_is_sent_with_body(
    world: &mut AppWorld,
    step: &Step,
    method: String,
    path: String,
) {
    world
        .send(&method, &path, Some("application/json"), doc_string(step))
        .await;
}

#[when(expr = "{word} request is sent to {string} with content type {string} and body:")]
async fn request_is_sent_with_content_type(
    world: &mut AppWorld,
    step: &Step,
    method: String,
    path: String,
    content_type: String,
) {
    world
        .send(&method, &path, Some(&content_type), doc_string(step))
        .await;
}

#[then(expr = "the response status code should be {int}")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
fn status_should_be(world: &mut AppWorld, expected: u16) {
    assert_eq!(
        world.reply().status,
        expected,
        "body: {}",
        world.reply().body
    );
}

#[then(expr = "the response body should contain {string}")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "cucumber fixes the step signature"
)]
fn body_should_contain(world: &mut AppWorld, text: String) {
    let text = world.substitute(&text);
    let body = &world.reply().body;
    assert!(
        body.contains(&text),
        "the body does not contain {text:?}: {body}"
    );
}

#[then(expr = "the response header {string} should contain {string}")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "cucumber fixes the step signature"
)]
fn header_should_contain(world: &mut AppWorld, name: String, text: String) {
    let headers = &world.reply().headers;
    let values: Vec<&str> = headers
        .get_all(name.as_str())
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect();
    assert!(
        values.iter().any(|value| value.contains(&text)),
        "header {name:?} does not contain {text:?}: {values:?}"
    );
}

#[then(expr = "the response header {string} should be present")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "cucumber fixes the step signature"
)]
fn header_should_be_present(world: &mut AppWorld, name: String) {
    assert!(
        world.reply().headers.contains_key(name.as_str()),
        "header {name:?} is missing"
    );
}

#[then(expr = "the JSON field {string} should equal {string}")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "cucumber fixes the step signature"
)]
fn json_field_should_equal(world: &mut AppWorld, field: String, expected: String) {
    let json = world.reply().json();
    assert_eq!(
        json.get(field.as_str()).and_then(serde_json::Value::as_str),
        Some(expected.as_str()),
        "body: {}",
        world.reply().body
    );
}

#[then("the JSON array should contain exactly the created resources")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
fn array_should_contain_created(world: &mut AppWorld) {
    let json = world.reply().json();
    world.assert_exactly_created(&json);
}
