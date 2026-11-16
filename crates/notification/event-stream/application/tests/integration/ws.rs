//! The WebSocket route over a real socket (FR-8 to FR-11, FR-24, NFR-9, NFR-14).

use std::sync::Arc;
use std::time::Duration;

use event_stream_domain::TopicError;
use serde_json::{Value, json};
use test_support::topic::ScriptedTopicReplay;
use tokio::io::AsyncWriteExt as _;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::Frame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::{CloseCode, Data, OpCode};

use crate::support::{
    BODY_LIMIT, Client, Settings, barrier, bounded, frame, next, node, node_with, send, text, texts,
};

/// The largest inbound message and frame the server reads.
const MAX_MESSAGE_BYTES: usize = 32_768;

fn subscribe(types: &[&str]) -> Message {
    Message::text(json!({"action": "subscribe", "types": types}).to_string())
}

fn unsubscribe(types: &[&str]) -> Message {
    Message::text(json!({"action": "unsubscribe", "types": types}).to_string())
}

/// The id of a WebSocket frame of an event.
fn id_of(text: &str) -> u64 {
    let frame: Value = serde_json::from_str(text).unwrap();
    frame["id"].as_str().unwrap().parse().unwrap()
}

/// True when the connection ended without a close frame: the next read reports the end of the
/// stream or an error.
async fn ended_without_a_close_frame(client: &mut Client) -> bool {
    match next(client).await {
        None | Some(Err(_)) => true,
        Some(Ok(message)) => panic!("a message after the end: {message:?}"),
    }
}

#[tokio::test]
async fn an_upgrade_answers_101_and_each_event_of_the_type_arrives_as_one_text_frame() {
    let node = node(Settings::default()).await;
    let (mut client, status) = node.connect("?types=user.created", None).await;

    node.pass(901, "user.created");
    node.pass(902, "order.paid");
    node.pass(903, "user.created");

    assert_eq!(status, 101);
    assert_eq!(node.hub.subscriber_count(), 1);
    assert_eq!(
        texts(&mut client, 2).await,
        [frame(901, "user.created"), frame(903, "user.created")]
    );
    let parsed: Value = serde_json::from_str(&frame(901, "user.created")).unwrap();
    assert_eq!(
        parsed,
        json!({"id": "901", "type": "user.created", "data": {"n": 901}})
    );
}

#[tokio::test]
async fn without_types_a_subscribe_adds_a_type_and_an_unsubscribe_removes_it() {
    let node = node(Settings::default()).await;
    let (mut client, status) = node.connect("", None).await;
    assert_eq!(status, 101);
    assert_eq!(node.hub.type_count(), 0);

    send(&mut client, subscribe(&["order.paid"])).await;
    barrier(&mut client).await;
    node.pass(901, "order.paid");
    let first = text(&mut client).await;
    send(&mut client, unsubscribe(&["order.paid"])).await;
    send(&mut client, subscribe(&["user.created"])).await;
    barrier(&mut client).await;
    node.pass(902, "order.paid");
    node.pass(903, "user.created");

    assert_eq!(first, frame(901, "order.paid"));
    assert_eq!(text(&mut client).await, frame(903, "user.created"));
    assert_eq!(node.hub.type_count(), 1);
}

#[tokio::test]
async fn an_invalid_command_is_answered_and_the_connection_stays_open() {
    let node = node(Settings::default()).await;
    let (mut client, _status) = node.connect("?types=user.created", None).await;

    send(&mut client, Message::text("subscribe")).await;
    let invalid = text(&mut client).await;
    send(&mut client, Message::binary(vec![1_u8, 2, 3])).await;
    let binary = text(&mut client).await;
    send(&mut client, subscribe(&["Bad"])).await;
    let name = text(&mut client).await;
    node.pass(901, "user.created");

    assert_eq!(invalid, r#"{"error":"invalid command"}"#);
    assert_eq!(binary, r#"{"error":"binary frames are not supported"}"#);
    assert_eq!(name, r#"{"error":"invalid event type name"}"#);
    assert_eq!(text(&mut client).await, frame(901, "user.created"));
}

#[tokio::test]
async fn a_client_ping_is_answered_with_a_pong() {
    let node = node(Settings::default()).await;
    let (mut client, _status) = node.connect("?types=user.created", None).await;

    send(&mut client, Message::Ping("probe".into())).await;

    match next(&mut client).await {
        Some(Ok(Message::Pong(payload))) => assert_eq!(payload.as_ref(), b"probe"),
        other => panic!("not a pong: {other:?}"),
    }
}

#[tokio::test]
async fn the_server_pings_every_keep_alive() {
    let node = node(Settings {
        keep_alive: Duration::from_millis(200),
        ..Settings::default()
    })
    .await;
    let (mut client, _status) = node.connect("?types=user.created", None).await;

    for _ in 0..2 {
        match next(&mut client).await {
            Some(Ok(Message::Ping(payload))) => assert!(payload.is_empty()),
            other => panic!("not a ping: {other:?}"),
        }
    }
}

#[test]
fn the_node_limits_request_bodies_to_1_024_bytes() {
    assert_eq!(BODY_LIMIT, 1_024);
}

/// The node serves its routes behind a body limit of [`BODY_LIMIT`] bytes: `HTTP_MAX_BODY_BYTES`
/// does not apply to the messages of a WebSocket.
#[tokio::test]
async fn a_frame_of_32_768_bytes_is_read_and_answered() {
    let node = node(Settings::default()).await;
    let (mut client, _status) = node.connect("?types=user.created", None).await;

    send(&mut client, Message::text("x".repeat(MAX_MESSAGE_BYTES))).await;
    let text_frame = text(&mut client).await;
    send(&mut client, Message::binary(vec![0_u8; MAX_MESSAGE_BYTES])).await;
    let binary_frame = text(&mut client).await;

    assert_eq!(text_frame, r#"{"error":"the command is too large"}"#);
    assert_eq!(
        binary_frame,
        r#"{"error":"binary frames are not supported"}"#
    );
}

/// Behind the body limit of [`BODY_LIMIT`] bytes, like the frame test above.
#[tokio::test]
async fn a_message_of_32_768_bytes_in_two_frames_is_read_and_answered() {
    let node = node(Settings::default()).await;
    let (mut client, _status) = node.connect("?types=user.created", None).await;
    let half = "x".repeat(MAX_MESSAGE_BYTES / 2);

    send(
        &mut client,
        Message::Frame(Frame::message(
            half.clone(),
            OpCode::Data(Data::Text),
            false,
        )),
    )
    .await;
    send(
        &mut client,
        Message::Frame(Frame::message(half, OpCode::Data(Data::Continue), true)),
    )
    .await;

    assert_eq!(
        text(&mut client).await,
        r#"{"error":"the command is too large"}"#
    );
}

#[tokio::test]
async fn a_message_of_32_769_bytes_in_two_frames_ends_the_connection_without_a_close_frame() {
    let node = node(Settings::default()).await;
    let (mut client, _status) = node.connect("?types=user.created", None).await;
    let first = "x".repeat(MAX_MESSAGE_BYTES / 2);
    let second = "x".repeat(MAX_MESSAGE_BYTES / 2 + 1);

    send(
        &mut client,
        Message::Frame(Frame::message(first, OpCode::Data(Data::Text), false)),
    )
    .await;
    send(
        &mut client,
        Message::Frame(Frame::message(second, OpCode::Data(Data::Continue), true)),
    )
    .await;

    assert!(ended_without_a_close_frame(&mut client).await);
    bounded(node.hub.drained()).await;
}

#[tokio::test]
async fn a_frame_header_declaring_32_769_bytes_ends_the_connection_before_its_payload() {
    let node = node(Settings::default()).await;
    let (mut client, _status) = node.connect("?types=user.created", None).await;
    // FIN and text, then the mask bit with the 16-bit length 32 769, then a mask key; no payload.
    let header = [0x81_u8, 0xFE, 0x80, 0x01, 0, 0, 0, 0];

    bounded(client.get_mut().write_all(&header)).await.unwrap();
    bounded(client.get_mut().flush()).await.unwrap();

    assert!(ended_without_a_close_frame(&mut client).await);
    bounded(node.hub.drained()).await;
}

#[tokio::test]
async fn a_cursor_on_the_query_resumes_from_the_window_and_the_header_wins() {
    let node = node(Settings::default()).await;
    for id in 901..=1_000 {
        node.pass(id, "user.created");
    }

    let (mut by_parameter, _status) = node
        .connect("?types=user.created&lastEventId=950", None)
        .await;
    let (mut by_header, _status) = node
        .connect("?types=user.created&lastEventId=950", Some("990"))
        .await;

    let resumed: Vec<u64> = texts(&mut by_parameter, 50)
        .await
        .iter()
        .map(|text| id_of(text))
        .collect();
    let from_header: Vec<u64> = texts(&mut by_header, 10)
        .await
        .iter()
        .map(|text| id_of(text))
        .collect();
    assert_eq!(resumed, (951..=1_000).collect::<Vec<_>>());
    assert_eq!(from_header, (991..=1_000).collect::<Vec<_>>());
    node.pass(1_001, "user.created");
    assert_eq!(id_of(&text(&mut by_parameter).await), 1_001);
    assert_eq!(id_of(&text(&mut by_header).await), 1_001);
}

#[tokio::test]
async fn a_burst_larger_than_the_queue_disconnects_nobody_and_every_event_arrives_once_in_order() {
    let node = node(Settings {
        buffer: 4,
        ..Settings::default()
    })
    .await;
    let (mut first, _status) = node.connect("?types=user.created", None).await;
    let (mut second, _status) = node.connect("?types=user.created,order.paid", None).await;

    for id in 901..=1_000 {
        node.pass(
            id,
            if id % 2 == 0 {
                "order.paid"
            } else {
                "user.created"
            },
        );
    }

    let odd: Vec<u64> = texts(&mut first, 50)
        .await
        .iter()
        .map(|text| id_of(text))
        .collect();
    let all: Vec<u64> = texts(&mut second, 100)
        .await
        .iter()
        .map(|text| id_of(text))
        .collect();
    assert_eq!(odd, (901..=1_000).step_by(2).collect::<Vec<_>>());
    assert_eq!(all, (901..=1_000).collect::<Vec<_>>());
    assert_eq!(node.hub.subscriber_count(), 2);
}

#[tokio::test]
async fn at_shutdown_the_queued_frames_arrive_then_close_code_1001() {
    let node = node(Settings::default()).await;
    let (mut client, _status) = node.connect("?types=user.created", None).await;
    node.pass(901, "user.created");
    node.pass(902, "user.created");

    node.hub.close();

    assert_eq!(
        texts(&mut client, 2).await,
        [frame(901, "user.created"), frame(902, "user.created")]
    );
    match next(&mut client).await {
        Some(Ok(Message::Close(Some(close)))) => assert_eq!(close.code, CloseCode::Away),
        other => panic!("not the close frame 1001: {other:?}"),
    }
    assert!(next(&mut client).await.is_none());
    bounded(node.hub.drained()).await;
}

#[tokio::test]
async fn a_replay_that_cannot_read_kafka_closes_with_code_1011() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Err(TopicError::Unavailable("the broker is away".into())));
    let node = node_with(Settings::default(), Arc::new(replay.clone())).await;

    let (mut client, status) = node
        .connect("?types=user.created&lastEventId=10", None)
        .await;

    assert_eq!(status, 101);
    match next(&mut client).await {
        Some(Ok(Message::Close(Some(close)))) => assert_eq!(close.code, CloseCode::Error),
        other => panic!("not the close frame 1011: {other:?}"),
    }
    assert!(next(&mut client).await.is_none());
    bounded(node.hub.drained()).await;
}

#[tokio::test]
async fn a_client_close_frame_is_answered_and_ends_the_subscription() {
    let node = node(Settings::default()).await;
    let (mut client, _status) = node.connect("?types=user.created", None).await;

    bounded(client.close(Some(CloseFrame {
        code: CloseCode::Normal,
        reason: "done".into(),
    })))
    .await
    .unwrap();

    match next(&mut client).await {
        Some(Ok(Message::Close(Some(close)))) => assert_eq!(close.code, CloseCode::Normal),
        other => panic!("not the answer to the close frame: {other:?}"),
    }
    assert!(next(&mut client).await.is_none());
    bounded(node.hub.drained()).await;
}
