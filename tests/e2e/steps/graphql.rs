//! GraphQL operation and result steps, over JSON and over an event stream in the POST form and in
//! the GET form: the subscriptions of the event stream, their blocks and their resume.

use std::time::Duration;

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use serde_json::{Value, json};

use crate::world::{AppWorld, Produced};

/// The bytes of the keep-alive comment block (FR-17).
const KEEP_ALIVE: &str = ":\n\n";

/// The bytes of the block that follows a single result, with its bare `data:` line (FR-8).
const COMPLETE: &str = "event: complete\ndata:\n\n";

/// The code of a validation error of the GraphQL path.
const VALIDATION_FAILED: &str = "VALIDATION_FAILED";

/// The subscription of the one type `event_type` with the selection `{ id type data }`, with
/// `last_event_id` as the argument `lastEventId` when given.
fn subscription(event_type: &str, last_event_id: Option<&str>) -> String {
    let cursor = last_event_id.map_or_else(String::new, |id| format!(", lastEventId: \"{id}\""));
    format!("subscription {{ events(types: [\"{event_type}\"]{cursor}) {{ id type data }} }}")
}

/// The subscription of the type list `types`, a GraphQL list literal, with the extra arguments
/// `arguments`.
fn events_of(types: &str, arguments: &str) -> String {
    format!("subscription {{ events(types: {types}{arguments}) {{ id }} }}")
}

/// The GraphQL list literal of `count` distinct valid type names.
fn distinct_names(count: usize) -> String {
    let names: Vec<String> = (0..count).map(|number| format!("\"t{number}\"")).collect();
    format!("[{}]", names.join(", "))
}

/// The document and the `Last-Event-ID` header of the invalid subscription named `case` (AC4).
fn invalid_subscription(case: &str) -> (String, Option<&'static str>) {
    let one_type = r#"["user.created"]"#;
    match case {
        "an invalid type name" => (events_of(r#"["User.Created"]"#, ""), None),
        "101 distinct type names" => (events_of(&distinct_names(101), ""), None),
        "an empty list of types" => (events_of("[]", ""), None),
        "a malformed lastEventId" => (events_of(one_type, r#", lastEventId: "x1""#), None),
        "a malformed Last-Event-ID" => (events_of(one_type, ""), Some("x1")),
        "a malformed lastEventId beside a valid Last-Event-ID" => {
            (events_of(one_type, r#", lastEventId: "x1""#), Some("1"))
        }
        "two root fields" => (
            format!(
                "subscription {{ a: events(types: {one_type}) {{ id }} b: events(types: {one_type}) {{ id }} }}"
            ),
            None,
        ),
        other => panic!("no invalid subscription is named {other:?}"),
    }
}

/// The `next` block of the subscription result of `event` for the selection `{ id type data }`:
/// its id line, then the execution result as compact JSON (FR-8, FR-10).
fn next_block(event: &Produced) -> String {
    let id = event.cursor;
    format!(
        "event: next\nid: {id}\ndata: {{\"data\":{{\"events\":{{\"id\":\"{id}\",\"type\":\"{}\",\"data\":{}}}}}}}\n\n",
        event.event_type, event.object
    )
}

/// Sends `document` to the node as a GraphQL request over an event stream in `form`, `POST` or
/// `GET`, with `last_event_id` in the `Last-Event-ID` header when given.
async fn send_in_form(
    world: &mut AppWorld,
    form: &str,
    document: &str,
    last_event_id: Option<&str>,
) {
    match form {
        "POST" => world.open_graphql_sse_post(document, last_event_id).await,
        "GET" => world.open_graphql_sse_get(document, last_event_id).await,
        other => panic!("no GraphQL form is named {other:?}: POST or GET"),
    }
}

/// The last answer is `200 text/event-stream` and its stream is open (FR-7).
fn assert_event_stream(world: &AppWorld) {
    let reply = world.reply();
    assert_eq!(reply.status, 200, "body: {}", reply.body);
    assert!(reply.is_event_stream(), "headers: {:?}", reply.headers);
    assert!(world.graphql_stream_is_open(), "no GraphQL stream is open");
}

/// The result of the next block, which must be a `next` block without an `id` line: a single
/// result (FR-8).
async fn single_result(world: &mut AppWorld) -> Value {
    let block = world
        .next_graphql_block()
        .await
        .expect("the GraphQL stream holds a next block");
    assert_eq!(
        block.event.as_deref(),
        Some("next"),
        "block: {:?}",
        block.text
    );
    assert_eq!(block.id, None, "block: {:?}", block.text);
    assert!(block.comments.is_empty(), "block: {:?}", block.text);
    let data = block.data.expect("a next block has a data line");
    serde_json::from_str(&data).unwrap_or_else(|_error| panic!("the result is not JSON: {data}"))
}

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
        json.is_object() && json.get("errors").is_none_or(Value::is_null),
        "body: {}",
        world.reply().body
    );
}

#[given(expr = "the GraphQL event-stream requests come from the origin {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "cucumber fixes the step signature"
)]
fn event_stream_requests_come_from_origin(world: &mut AppWorld, origin: String) {
    world.open_graphql_sse_from_origin(&origin);
}

#[given(expr = "a GraphQL subscription of {string} is open in the {word} form")]
async fn subscription_is_open(world: &mut AppWorld, event_type: String, form: String) {
    send_in_form(world, &form, &subscription(&event_type, None), None).await;
    assert_event_stream(world);
}

#[when(expr = "a GraphQL subscription of {string} is sent in the {word} form")]
async fn subscription_is_sent(world: &mut AppWorld, event_type: String, form: String) {
    send_in_form(world, &form, &subscription(&event_type, None), None).await;
}

#[when(expr = "a GraphQL subscription with {string} is sent in the {word} form")]
async fn invalid_subscription_is_sent(world: &mut AppWorld, case: String, form: String) {
    let (document, last_event_id) = invalid_subscription(&case);
    send_in_form(world, &form, &document, last_event_id).await;
}

#[when(expr = "the GraphQL operation is sent in the {word} form as an event stream:")]
async fn operation_is_sent_as_event_stream(world: &mut AppWorld, step: &Step, form: String) {
    let document = step
        .docstring
        .as_deref()
        .expect("the step has a doc string");
    send_in_form(world, &form, document, None).await;
}

#[when("the GraphQL subscriber disconnects")]
fn subscriber_disconnects(world: &mut AppWorld) {
    world.disconnect_graphql();
}

#[when(
    expr = "the GraphQL subscriber of {string} resubscribes in the {word} form with the last id it received in the argument lastEventId"
)]
async fn resubscribes_by_argument(world: &mut AppWorld, event_type: String, form: String) {
    let last_id = world.graphql_last_id();
    let document = subscription(&event_type, Some(&last_id));
    send_in_form(world, &form, &document, None).await;
    assert_event_stream(world);
}

#[when(
    expr = "the GraphQL subscriber of {string} resubscribes in the {word} form with the last id it received in the header Last-Event-ID"
)]
async fn resubscribes_by_header(world: &mut AppWorld, event_type: String, form: String) {
    let last_id = world.graphql_last_id();
    let document = subscription(&event_type, None);
    send_in_form(world, &form, &document, Some(&last_id)).await;
    assert_event_stream(world);
}

#[then(
    expr = "the GraphQL subscriber receives the produced {string} events as next blocks with their id"
)]
async fn subscriber_receives(world: &mut AppWorld, event_type: String) {
    for expected in world.take_unchecked(&event_type) {
        let block = world.next_graphql_result().await;
        assert_eq!(block.text, next_block(&expected));
    }
}

#[then("the GraphQL answer is an event stream")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
fn answer_is_event_stream(world: &mut AppWorld) {
    assert_event_stream(world);
}

#[then("the GraphQL answer has no event-stream head")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
fn answer_has_no_event_stream_head(world: &mut AppWorld) {
    let reply = world.reply();
    assert!(!reply.is_event_stream(), "headers: {:?}", reply.headers);
    assert!(!world.graphql_stream_is_open(), "a GraphQL stream is open");
}

#[then("the GraphQL stream reads a comment")]
async fn stream_reads_comment(world: &mut AppWorld) {
    let block = world
        .next_graphql_block()
        .await
        .expect("the GraphQL stream holds a comment");
    assert_eq!(block.text, KEEP_ALIVE);
    assert!(block.is_comment());
}

/// Both bounds are measured from when the request was sent. The body starts its keep-alive timer
/// after that, so a comment due `earliest` seconds after the body started never comes earlier;
/// the read stops `latest` seconds after the request, so a body that does not use the configured
/// interval fails within that real-time wait instead of passing (FR-17).
#[then(
    expr = "the GraphQL stream reads a comment between {int} and {int} seconds after the request was sent"
)]
async fn stream_reads_comment_between(world: &mut AppWorld, earliest: u64, latest: u64) {
    let block = world
        .next_graphql_block_within(Duration::from_secs(latest))
        .await
        .expect("the GraphQL stream holds a comment");
    let elapsed = world.since_graphql_sent();
    assert_eq!(block.text, KEEP_ALIVE);
    assert!(
        elapsed >= Duration::from_secs(earliest),
        "the comment came {elapsed:?} after the request"
    );
}

#[then(
    expr = "the GraphQL stream reads one next block whose error is VALIDATION_FAILED on {string} with the code {string}"
)]
async fn stream_reads_validation_error(world: &mut AppWorld, path: String, code: String) {
    let result = single_result(world).await;
    let errors = result["errors"]
        .as_array()
        .unwrap_or_else(|| panic!("no errors in {result}"));
    assert_eq!(errors.len(), 1, "result: {result}");
    let extensions = &errors[0]["extensions"];
    assert_eq!(extensions["code"], VALIDATION_FAILED, "result: {result}");
    let violations = extensions["violations"]
        .as_array()
        .unwrap_or_else(|| panic!("no violations in {result}"));
    assert_eq!(violations.len(), 1, "result: {result}");
    assert_eq!(
        violations[0]["propertyPath"],
        path.as_str(),
        "result: {result}"
    );
    assert_eq!(violations[0]["code"], code.as_str(), "result: {result}");
    assert!(
        result.get("data").is_none_or(Value::is_null),
        "result: {result}"
    );
}

#[then("the GraphQL stream reads one next block with the result:")]
async fn stream_reads_result(world: &mut AppWorld, step: &Step) {
    let expected: Value = serde_json::from_str(
        step.docstring
            .as_deref()
            .expect("the step has a doc string"),
    )
    .expect("the doc string is JSON");
    assert_eq!(single_result(world).await, expected);
}

#[then("the GraphQL stream reads the complete block with a bare data line")]
async fn stream_reads_complete(world: &mut AppWorld) {
    let block = world
        .next_graphql_block()
        .await
        .expect("the GraphQL stream holds the complete block");
    assert_eq!(block.text, COMPLETE);
}

#[then("the GraphQL stream ends")]
async fn stream_ends(world: &mut AppWorld) {
    let block = world.next_graphql_block().await;
    assert_eq!(block, None, "the GraphQL stream goes on");
    world.disconnect_graphql();
}

#[given(expr = "the status of the node reports {int} subscriber(s)")]
#[then(expr = "the status of the node reports {int} subscriber(s)")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
async fn status_reports_subscribers(world: &mut AppWorld, count: u64) {
    assert_eq!(world.subscribers().await, count);
}
