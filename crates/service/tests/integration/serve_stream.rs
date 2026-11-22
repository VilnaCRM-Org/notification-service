//! `service serve` as a process against the broker of the stack (FR-17, FR-20, FR-21, NFR-6): a
//! topic that does not exist yet, a topic of three partitions, and a broker outage produced by a
//! forwarder of the test, because stopping the shared broker would break the tests that run in
//! parallel.

use std::time::Duration;

use serde_json::{Value, json};

use crate::broker::{
    Forwarder, TYPE, TestTopic, bootstrap_servers, envelope_id, envelopes, unique_name,
};
use crate::client::{Sse, envelope_ids, get, ids, local, status, subscription, wait_for};
use crate::support::{Answer, Scratch, messages_of, serve};

/// How long a test lets a starting node fail its rounds: about ten rounds with
/// `KAFKA_RETRY_DELAY` 100 ms. A process has no paused time, and that a fault is logged once is
/// only shown over rounds that fail, so the wait is bounded by its fixed length.
const FAILED_ROUNDS: Duration = Duration::from_secs(1);

/// The configuration of a served binary on `topic` through `bootstrap`: one event type, short
/// pauses after a failed round and a keep-alive every second.
fn variables<'a>(bootstrap: &'a str, topic: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("KAFKA_BOOTSTRAP_SERVERS", bootstrap),
        ("KAFKA_TOPIC", topic),
        ("EVENT_TYPES", TYPE),
        ("KAFKA_RETRY_DELAY", "100ms"),
        ("STREAM_KEEP_ALIVE", "1s"),
    ]
}

/// The path of the route `route` with a subscription of [`TYPE`].
fn route(route: &str) -> String {
    format!("/api/events/{route}{}", subscription(TYPE))
}

/// Asserts the 503 problem of a node that is starting.
fn assert_starting(answer: &Answer) {
    assert_eq!(answer.status, 503, "{}", answer.body);
    assert!(
        answer
            .head
            .contains("content-type: application/problem+json"),
        "{}",
        answer.head
    );
    let problem: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(problem["detail"], "the node is starting", "{problem}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_served_binary_waits_for_a_missing_topic_and_becomes_ready_once_it_exists() {
    let scratch = Scratch::new();
    let name = unique_name("serve-missing");
    let bootstrap = bootstrap_servers();
    let mut server = serve(&scratch, &variables(&bootstrap, &name));
    let invalid = server.read_until("event topic invalid").pop().unwrap();
    let address = local(server.port);
    let starting = status(address).await;
    let stream = get(address, &route("stream")).await;
    let socket = get(address, &route("ws")).await;
    tokio::time::sleep(FAILED_ROUNDS).await;

    let mut topic = TestTopic::create_named(name, 1).await;
    let ready_entry = server.read_until("event tail ready").pop().unwrap();
    let ready = wait_for(address, "state", &json!("ready")).await;
    let finished = server.terminate();
    topic.delete().await;
    drop(topic);

    assert_eq!(invalid["level"], "ERROR");
    assert_eq!(invalid["reason"], "topic-not-found");
    assert_eq!(starting["state"], "starting");
    assert_eq!(starting["reason"], "topic-not-found");
    assert_starting(&stream);
    assert_starting(&socket);
    assert_eq!(ready_entry["level"], "INFO");
    assert_eq!(ready_entry["position"], 0);
    assert_eq!(ready["reason"], Value::Null);
    assert_eq!(
        messages_of(&finished.lines, "event topic invalid").len(),
        1,
        "{:?}",
        finished.lines
    );
    assert_eq!(messages_of(&finished.lines, "event tail ready").len(), 1);
    assert_eq!(finished.code, Some(0));
    assert_eq!(finished.stderr, "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_served_binary_on_a_topic_of_three_partitions_stays_starting() {
    let mut topic = TestTopic::create("serve-partitions", 3).await;
    let scratch = Scratch::new();
    let bootstrap = bootstrap_servers();
    let mut server = serve(&scratch, &variables(&bootstrap, &topic.name));
    let invalid = server.read_until("event topic invalid").pop().unwrap();
    let address = local(server.port);
    tokio::time::sleep(FAILED_ROUNDS).await;

    let later = status(address).await;
    let stream = get(address, &route("stream")).await;
    let finished = server.terminate();
    topic.delete().await;
    drop(topic);

    assert_eq!(invalid["level"], "ERROR");
    assert_eq!(invalid["reason"], "partition-count");
    assert_eq!(later["state"], "starting");
    assert_eq!(later["reason"], "partition-count");
    assert_starting(&stream);
    assert_eq!(
        messages_of(&finished.lines, "event topic invalid").len(),
        1,
        "{:?}",
        finished.lines
    );
    assert_eq!(messages_of(&finished.lines, "event tail ready").len(), 0);
    assert_eq!(finished.code, Some(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ready_binary_keeps_its_streams_through_a_broker_outage_and_continues_from_its_position()
{
    let mut topic = TestTopic::create("serve-outage", 1).await;
    let mut forwarder = Forwarder::start(&topic).await;
    let scratch = Scratch::new();
    let bootstrap = forwarder.address().to_string();
    let server = serve(&scratch, &variables(&bootstrap, &topic.name));
    let address = local(server.port);
    wait_for(address, "state", &json!("ready")).await;
    let mut open = Sse::open(address, &subscription(TYPE), None).await;
    topic.produce(&envelopes(1..=3)).await;
    let before = open.events(3).await;

    forwarder.stop().await;
    let away = wait_for(address, "brokerReachable", &json!(false)).await;
    let mut joined = Sse::open(address, &subscription(TYPE), None).await;
    // The keep-alive comes a second after the last frame, so both are written during the outage.
    let quiet = open.keep_alives(2).await;
    topic.produce(&envelopes(4..=6)).await;
    forwarder.restart().await;
    let back = wait_for(address, "brokerReachable", &json!(true)).await;
    let after = open.events(3).await;
    let late = joined.events(3).await;
    let rests = [open.keep_alives(1).await, joined.keep_alives(1).await];
    let finished = server.terminate();
    topic.delete().await;
    drop(topic);

    assert_eq!(ids(&before), [1, 2, 3]);
    assert_eq!(away["state"], "ready");
    assert!(quiet.is_empty(), "{quiet:?}");
    assert_eq!(ids(&after), [4, 5, 6], "no id is repeated after the outage");
    let expected: Vec<String> = (4..=6).map(envelope_id).collect();
    assert_eq!(envelope_ids(&after), expected);
    assert_eq!(ids(&late), [4, 5, 6]);
    for rest in rests {
        assert!(rest.is_empty(), "no id follows the last event: {rest:?}");
    }
    assert_eq!(back["state"], "ready");
    let messages = finished.messages();
    let failed = messages
        .iter()
        .position(|message| *message == "event tail failed")
        .expect("the outage is logged");
    let recovered = messages
        .iter()
        .position(|message| *message == "event tail recovered")
        .expect("the recovery is logged");
    assert!(failed < recovered, "{messages:?}");
    assert_eq!(finished.code, Some(0));
}
