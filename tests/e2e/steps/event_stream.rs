//! The steps of the event stream: the node of a scenario and its topic, the records the scenario
//! writes, the status of the node, and the SSE and WebSocket clients that read the events.

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use serde_json::{Value, json};

use crate::world::{AppWorld, Produced, SseEvent};

/// The SSE frame of `event`: its id, its type and its event object.
fn sse_event(event: &Produced) -> SseEvent {
    SseEvent {
        id: Some(event.cursor.to_string()),
        event: Some(event.event_type.clone()),
        data: Some(event.object.clone()),
    }
}

/// The WebSocket text frame of `event`: its id as a string, its type and its event object.
fn websocket_frame(event: &Produced) -> String {
    format!(
        r#"{{"id":"{}","type":"{}","data":{}}}"#,
        event.cursor, event.event_type, event.object
    )
}

/// The WebSocket command `action` for the one type `event_type`.
fn command(action: &str, event_type: &str) -> String {
    json!({"action": action, "types": [event_type]}).to_string()
}

/// The value goes to the argument of the serve command that reads the variable; a variable that
/// no argument reads, or that the suite sets itself, fails the scenario.
#[given(expr = "the environment variable {string} is set to {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "cucumber fixes the step signature"
)]
fn variable_is_set(world: &mut AppWorld, name: String, value: String) {
    world.set_variable(&name, &value);
}

#[given("a ready node with a topic of its own")]
async fn ready_node(world: &mut AppWorld) {
    world.start_ready_node().await;
}

#[given("a node whose topic does not exist yet")]
async fn node_without_topic(world: &mut AppWorld) {
    world.start_node_without_topic().await;
}

#[when(expr = "a(n) {string} event is produced")]
async fn event_is_produced(world: &mut AppWorld, event_type: String) {
    world.produce_event(&event_type).await;
}

#[when(expr = "{int} {string} events are produced")]
async fn events_are_produced(world: &mut AppWorld, count: usize, event_type: String) {
    for _ in 0..count {
        world.produce_event(&event_type).await;
    }
}

#[when("these events are produced:")]
async fn these_events_are_produced(world: &mut AppWorld, step: &Step) {
    let table = step.table.as_ref().expect("the step has a table");
    assert_eq!(table.rows[0], ["type"], "the table has the one column type");
    for row in &table.rows[1..] {
        world.produce_event(&row[0]).await;
    }
}

#[when("a record that is not an event is produced")]
async fn invalid_record_is_produced(world: &mut AppWorld) {
    world.produce_invalid().await;
}

#[when("the node has read every produced event")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
async fn node_has_read_every_event(world: &mut AppWorld) {
    world.wait_until_read().await;
}

#[then("the window of the node no longer holds the first produced event")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
async fn window_no_longer_holds_the_first_event(world: &mut AppWorld) {
    let first = world
        .produced()
        .first()
        .expect("the scenario produced an event")
        .cursor;
    let floor = world.window_floor().await;
    assert!(
        floor >= first,
        "the window holds the events above {floor}, the first event is {first}"
    );
}

#[given(expr = "the status of the node reports {int} rejected record(s)")]
#[then(expr = "the status of the node reports {int} rejected record(s)")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
async fn status_reports_rejected_records(world: &mut AppWorld, count: u64) {
    let expected = count.to_string();
    world
        .wait_for_status(&format!("{count} rejected records"), |status| {
            status.get("rejectedRecords").and_then(Value::as_str) == Some(expected.as_str())
        })
        .await;
}

#[given(expr = "the status of the node reports the state starting with the reason {string}")]
#[expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber fixes the step signature"
)]
async fn status_reports_starting_reason(world: &mut AppWorld, reason: String) {
    world
        .wait_for_status(
            &format!("the state starting with the reason {reason}"),
            |status| status["state"] == "starting" && status["reason"] == reason.as_str(),
        )
        .await;
}

#[given(expr = "an SSE subscriber of {string}")]
async fn sse_subscriber(world: &mut AppWorld, types: String) {
    world
        .open_sse(&format!("/api/events/stream?types={types}"), None)
        .await;
}

#[when(
    expr = "an SSE subscriber of {string} resumes from the id {string} in the query parameter lastEventId"
)]
async fn sse_subscriber_resumes(world: &mut AppWorld, types: String, id: String) {
    world
        .open_sse(
            &format!("/api/events/stream?types={types}&lastEventId={id}"),
            None,
        )
        .await;
}

#[when("the SSE subscriber disconnects")]
fn sse_subscriber_disconnects(world: &mut AppWorld) {
    world.disconnect_sse();
}

#[when("the SSE subscriber reconnects with the header Last-Event-ID of the last id it received")]
async fn sse_subscriber_reconnects(world: &mut AppWorld) {
    world.reconnect_sse().await;
}

#[then(
    expr = "the SSE subscriber receives the produced {string} events with their id, event and data fields"
)]
async fn sse_subscriber_receives(world: &mut AppWorld, event_type: String) {
    for expected in world.take_unchecked(&event_type) {
        assert_eq!(world.next_sse_event().await, sse_event(&expected));
    }
}

#[given("a WebSocket without types")]
async fn websocket_without_types(world: &mut AppWorld) {
    world.open_websocket("/api/events/ws").await;
}

#[when(expr = "the WebSocket subscribes to {string}")]
async fn websocket_subscribes(world: &mut AppWorld, event_type: String) {
    world.send_command(command("subscribe", &event_type)).await;
}

#[when(expr = "the WebSocket unsubscribes from {string}")]
async fn websocket_unsubscribes(world: &mut AppWorld, event_type: String) {
    world
        .send_command(command("unsubscribe", &event_type))
        .await;
}

#[then(expr = "the WebSocket receives the produced {string} events as JSON text frames")]
async fn websocket_receives(world: &mut AppWorld, event_type: String) {
    for expected in world.take_unchecked(&event_type) {
        let text = world.next_text().await;
        assert!(
            serde_json::from_str::<Value>(&text).is_ok_and(|frame| frame.is_object()),
            "not a JSON object: {text}"
        );
        assert_eq!(text, websocket_frame(&expected));
    }
}
