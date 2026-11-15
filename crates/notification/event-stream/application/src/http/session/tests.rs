use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes};
use event_stream_domain::{Cursor, TopicError};
use rstest::rstest;
use test_support::topic::ScriptedTopicReplay;
use tokio::time::timeout;

use super::{Inbound, Session};
use crate::http::subscription::{EndReason, Step};
use crate::http::test_node::{Node, node, node_with};

/// Bounds every await of these tests (time is paused, so nothing really waits).
const WAIT: Duration = Duration::from_secs(5);

const TOO_LARGE: &str = r#"{"error":"the command is too large"}"#;
const INVALID_COMMAND: &str = r#"{"error":"invalid command"}"#;
const TYPES_COUNT: &str = r#"{"error":"types must hold 1 to 100 event type names"}"#;
const INVALID_NAME: &str = r#"{"error":"invalid event type name"}"#;
const SET_LIMIT: &str = r#"{"error":"a connection subscribes to at most 100 event types"}"#;
const BINARY: &str = r#"{"error":"binary frames are not supported"}"#;

/// `count` distinct valid names `t0`, `t1`, ...
fn distinct(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("t{index}")).collect()
}

/// A live session of `types` on `node`.
fn session_of(node: &Node, types: &[String]) -> Session {
    let types: Vec<&str> = types.iter().map(String::as_str).collect();
    node.session(&types, None)
}

fn text(text: &str) -> Message {
    Message::Text(Utf8Bytes::from(text))
}

/// The text of the reply, or a panic when the session did not reply.
fn reply(inbound: Inbound) -> String {
    match inbound {
        Inbound::Reply(Message::Text(text)) => text.as_str().to_owned(),
        Inbound::Reply(other) => panic!("a reply that is not text: {other:?}"),
        Inbound::Continue => panic!("no reply"),
        Inbound::Close => panic!("a close"),
    }
}

fn subscribe(types: &[String]) -> String {
    serde_json::json!({"action": "subscribe", "types": types}).to_string()
}

fn unsubscribe(types: &[String]) -> String {
    serde_json::json!({"action": "unsubscribe", "types": types}).to_string()
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// One step within `WAIT`; panics when it pends.
async fn step(session: &mut Session) -> Step {
    timeout(WAIT, session.step())
        .await
        .expect("a step within the wait")
}

/// The id of the next frame, after at most a few idle steps.
async fn next_id(session: &mut Session) -> u64 {
    for _ in 0..8 {
        match step(session).await {
            Step::Frame(frame) => return frame.sequence.value(),
            Step::Idle => {}
            Step::End(reason) => panic!("the session ended: {reason:?}"),
        }
    }
    panic!("no frame within the idle steps")
}

#[test]
fn the_largest_command_is_16_384_bytes() {
    assert_eq!(Session::MAX_COMMAND_BYTES, 16_384);
}

#[tokio::test(start_paused = true)]
async fn a_subscribe_command_is_not_answered_and_adds_its_types() {
    let node = node();
    let mut session = node.session(&[], None);

    let inbound = session.on_message(text(&subscribe(&owned(&["order.paid", "user.created"]))));

    assert!(matches!(inbound, Inbound::Continue));
    assert_eq!(session.type_count(), 2);
    assert_eq!(node.hub.type_count(), 2);
    node.pass(1_001, "order.paid");
    assert_eq!(next_id(&mut session).await, 1_001);
}

#[tokio::test(start_paused = true)]
async fn an_unsubscribe_command_is_not_answered_and_removes_its_types() {
    let node = node();
    let mut session = node.session(&["order.paid", "user.created"], None);
    node.pass(1_001, "order.paid");

    let inbound = session.on_message(text(&unsubscribe(&owned(&["order.paid", "absent"]))));

    assert!(matches!(inbound, Inbound::Continue));
    assert_eq!(session.type_count(), 1);
    assert_eq!(node.hub.type_count(), 1);
    node.pass(1_002, "user.created");
    assert_eq!(next_id(&mut session).await, 1_002);
}

#[tokio::test(start_paused = true)]
async fn a_command_of_16_384_bytes_is_read_and_one_byte_more_is_too_large() {
    let node = node();
    let mut session = node.session(&[], None);
    let command = subscribe(&owned(&["user.created"]));
    let padding = Session::MAX_COMMAND_BYTES - command.len();
    let largest = format!("{}{command}", " ".repeat(padding));
    let too_large = format!(" {largest}");

    let refused = reply(session.on_message(text(&too_large)));
    let unchanged = session.type_count();
    let accepted = session.on_message(text(&largest));

    assert_eq!(largest.len(), 16_384);
    assert_eq!(refused, TOO_LARGE);
    assert_eq!(unchanged, 0);
    assert!(matches!(accepted, Inbound::Continue));
    assert_eq!(session.type_count(), 1);
}

#[rstest]
#[case::not_json("subscribe")]
#[case::unknown_action(r#"{"action":"replay","types":["a"]}"#)]
#[case::unknown_member(r#"{"action":"subscribe","types":["a"],"since":"1"}"#)]
#[case::missing_types(r#"{"action":"subscribe"}"#)]
#[case::bad_name_and_unknown_member(r#"{"action":"subscribe","types":["A"],"x":1}"#)]
#[tokio::test(start_paused = true)]
async fn a_text_that_is_not_a_command_is_an_invalid_command(#[case] command: &str) {
    let node = node();
    let mut session = node.session(&["user.created"], None);

    assert_eq!(reply(session.on_message(text(command))), INVALID_COMMAND);
    assert_eq!(session.type_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn the_size_is_checked_before_the_shape() {
    let node = node();
    let mut session = node.session(&[], None);
    let not_json = "x".repeat(16_385);

    assert_eq!(reply(session.on_message(text(&not_json))), TOO_LARGE);
}

#[rstest]
#[case::subscribe_empty(subscribe(&[]), TYPES_COUNT)]
#[case::unsubscribe_empty(unsubscribe(&[]), TYPES_COUNT)]
#[case::subscribe_101(subscribe(&distinct(101)), TYPES_COUNT)]
#[case::unsubscribe_101(unsubscribe(&distinct(101)), TYPES_COUNT)]
#[case::subscribe_pattern(subscribe(&owned(&["User.Created"])), INVALID_NAME)]
#[case::subscribe_blank(subscribe(&owned(&["a", ""])), INVALID_NAME)]
#[case::subscribe_length(subscribe(&["a".repeat(129)]), INVALID_NAME)]
#[case::unsubscribe_pattern(unsubscribe(&owned(&["user.created", "Order"])), INVALID_NAME)]
#[tokio::test(start_paused = true)]
async fn invalid_names_are_answered_and_change_nothing(
    #[case] command: String,
    #[case] expected: &str,
) {
    let node = node();
    let mut session = node.session(&["user.created"], None);

    assert_eq!(reply(session.on_message(text(&command))), expected);
    assert_eq!(session.type_count(), 1);
    assert_eq!(node.hub.type_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_hundred_distinct_names_with_repeats_are_one_command() {
    let node = node();
    let mut session = node.session(&[], None);
    let mut names = distinct(100);
    names.extend(distinct(100));

    let inbound = session.on_message(text(&subscribe(&names)));

    assert!(matches!(inbound, Inbound::Continue));
    assert_eq!(session.type_count(), 100);
}

#[tokio::test(start_paused = true)]
async fn a_subscribe_that_would_exceed_100_types_is_answered_and_changes_nothing() {
    let node = node();
    let mut session = session_of(&node, &distinct(99));
    let two_new = owned(&["new.one", "new.two"]);

    let refused = reply(session.on_message(text(&subscribe(&two_new))));
    let count = session.type_count();
    let one_new = session.on_message(text(&subscribe(&owned(&["new.one"]))));

    assert_eq!(refused, SET_LIMIT);
    assert_eq!(count, 99);
    assert!(matches!(one_new, Inbound::Continue));
    assert_eq!(session.type_count(), 100);
    assert_eq!(node.hub.type_count(), 100);
}

#[tokio::test(start_paused = true)]
async fn the_names_are_checked_before_the_limit_of_the_set() {
    let node = node();
    let mut session = session_of(&node, &distinct(100));

    let bad_name = reply(session.on_message(text(&subscribe(&owned(&["Bad"])))));
    let empty = reply(session.on_message(text(&subscribe(&[]))));
    let held = session.on_message(text(&subscribe(&owned(&["t0"]))));

    assert_eq!(bad_name, INVALID_NAME);
    assert_eq!(empty, TYPES_COUNT);
    assert!(matches!(held, Inbound::Continue));
    assert_eq!(session.type_count(), 100);
}

#[tokio::test(start_paused = true)]
async fn a_binary_frame_is_answered_and_changes_nothing() {
    let node = node();
    let mut session = node.session(&["user.created"], None);
    let command = subscribe(&owned(&["order.paid"]));

    let inbound = session.on_message(Message::Binary(Bytes::from(command)));

    assert_eq!(reply(inbound), BINARY);
    assert_eq!(session.type_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn pings_and_pongs_change_nothing_and_are_not_answered_by_the_session() {
    let node = node();
    let mut session = node.session(&["user.created"], None);

    let messages = [
        Message::Ping(Bytes::from_static(b"p")),
        Message::Pong(Bytes::from_static(b"p")),
    ];

    for message in messages {
        assert!(matches!(session.on_message(message), Inbound::Continue));
    }
    assert_eq!(session.type_count(), 1);
}

#[rstest]
#[case::without_a_frame(None)]
#[case::with_a_frame(Some(CloseFrame { code: 1000, reason: Utf8Bytes::from_static("bye") }))]
#[tokio::test(start_paused = true)]
async fn a_close_frame_closes_the_session(#[case] frame: Option<CloseFrame>) {
    let node = node();
    let mut session = node.session(&["user.created"], None);

    assert!(matches!(
        session.on_message(Message::Close(frame)),
        Inbound::Close
    ));
}

#[rstest]
#[case::hub_closed(EndReason::HubClosed, 1_001)]
#[case::topic_failed(EndReason::TopicFailed, 1_011)]
fn each_end_has_its_close_code(#[case] reason: EndReason, #[case] code: u16) {
    assert_eq!(
        Session::close_message(reason),
        Message::Close(Some(CloseFrame {
            code,
            reason: Utf8Bytes::from_static(""),
        }))
    );
}

#[tokio::test(start_paused = true)]
async fn the_session_steps_its_subscription_and_reports_its_position() {
    let node = node();
    let mut session = node.session(&["user.created"], None);
    node.pass(1_001, "order.paid");
    node.pass(1_002, "user.created");

    let first = session.position();
    let id = next_id(&mut session).await;

    assert_eq!(first, Cursor::new(1_000));
    assert_eq!(id, 1_002);
    assert_eq!(session.position(), Cursor::new(1_002));
    assert!(!session.replaying());
    assert_eq!(session.failure(), None);
}

#[tokio::test(start_paused = true)]
async fn a_cursor_below_the_window_replays_and_a_failed_read_is_kept() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Err(TopicError::Unavailable("the broker is away".into())));
    let node = node_with(Arc::new(replay.clone()));
    let mut session = node.session(&["user.created"], Some(10));

    let before = session.replaying();
    let start = step(&mut session).await;
    let replaying = session.replaying();
    let failed = step(&mut session).await;

    assert!(
        !before,
        "the first step decides the floor and reads nothing"
    );
    assert!(matches!(start, Step::Idle));
    assert!(replaying);
    assert!(matches!(failed, Step::End(EndReason::TopicFailed)));
    assert_eq!(replay.calls(), [Cursor::new(10)]);
    assert_eq!(
        session.failure(),
        Some("the topic is unavailable: the broker is away")
    );
}

#[tokio::test(start_paused = true)]
async fn a_live_step_below_the_window_idles_and_leaves_the_read_to_the_next_step() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Err(TopicError::Unavailable("the broker is away".into())));
    let node = node_with(Arc::new(replay.clone()));
    let mut session = node.session(&["user.created"], Some(10));

    let start = step(&mut session).await;
    let live = timeout(WAIT, session.live_step())
        .await
        .expect("a live step within the wait");
    let calls = replay.calls();
    let replaying = session.replaying();
    let failed = step(&mut session).await;

    assert!(matches!(start, Step::Idle));
    assert!(matches!(live, Step::Idle));
    assert_eq!(calls, Vec::<Cursor>::new());
    assert!(replaying);
    assert!(matches!(failed, Step::End(EndReason::TopicFailed)));
    assert_eq!(replay.calls(), [Cursor::new(10)]);
}
