//! Two nodes in process on one topic of the broker of the stack (FR-22, FR-23, FR-36): each is
//! the production wiring with its own Kafka readers, its own tail and its own hub, built from one
//! configuration that names no other node.

use std::net::SocketAddr;

use serde_json::json;
use service::background::Running;
use service::bootstrap::{Adapters, Application, application};
use service::cli::{DatabaseArgs, KafkaArgs, ServeArgs, StreamArgs};
use service::config::Config;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::broker::{TYPE, TestTopic, bootstrap_servers, envelope, envelope_id, envelopes};
use crate::client::{Sse, envelope_ids, ids, local, status, subscription, wait_for};
use crate::limit::{bounded, listen_on};
use crate::support::DEAD_DATABASE_URL;

/// The configuration of every node of a test: the topic and the broker, nothing about another
/// node, and a keep-alive every second, which ends the read that shows that nothing follows the
/// last expected event. The database and SQS are on port 1 and never reached (no request of these
/// tests needs them).
fn config(topic: &str) -> Config {
    Config::from_args(ServeArgs {
        aws_region: Some("us-east-1".to_owned()),
        aws_endpoint_url_sqs: Some("http://127.0.0.1:1".to_owned()),
        database: DatabaseArgs {
            url: Some(DEAD_DATABASE_URL.to_owned()),
            ..DatabaseArgs::default()
        },
        event_types: Some(TYPE.to_owned()),
        kafka: KafkaArgs {
            bootstrap_servers: Some(bootstrap_servers()),
            topic: Some(topic.to_owned()),
            retry_delay: Some("100ms".to_owned()),
            ..KafkaArgs::default()
        },
        streams: StreamArgs {
            keep_alive: Some("1s".to_owned()),
            ..StreamArgs::default()
        },
        ..ServeArgs::default()
    })
    .unwrap()
}

/// A node served in process on a loopback port: its router and its background work.
#[derive(Debug)]
struct Node {
    address: SocketAddr,
    running: Running,
    stop: oneshot::Sender<()>,
    server: JoinHandle<std::io::Result<()>>,
}

impl Node {
    /// Wires a node from `config` with the production adapters, serves it and starts its tail.
    async fn start(config: &Config) -> Self {
        let adapters = bounded(Box::pin(Adapters::connect(config))).await.unwrap();
        let Application { router, background } = application(adapters, config).unwrap();
        let listener = listen_on(local(0)).await;
        let address = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel::<()>();
        let serving = axum::serve(listener, router).with_graceful_shutdown(async move {
            stopped.await.unwrap_or_default();
        });
        let server = tokio::spawn(async move { serving.await });
        Self {
            address,
            running: background.spawn(),
            stop,
            server,
        }
    }

    /// Stops the node in the order of the serve command: new connections are refused, then its
    /// tail, its hub and its streams end, then the HTTP drain.
    async fn stop(self) {
        self.stop.send(()).unwrap();
        bounded(self.running.shutdown()).await;
        bounded(self.server).await.unwrap().unwrap();
    }
}

/// A new topic of one partition named after `test` and two nodes on it, built from one
/// configuration, each ready.
async fn cluster(test: &str) -> (TestTopic, Node, Node) {
    let topic = TestTopic::create(test, 1).await;
    let config = config(&topic.name);
    let first = Node::start(&config).await;
    let second = Node::start(&config).await;
    wait_for(first.address, "state", &json!("ready")).await;
    wait_for(second.address, "state", &json!("ready")).await;
    (topic, first, second)
}

/// Stops both nodes and removes the topic.
async fn tear_down(mut topic: TestTopic, first: Node, second: Node) {
    first.stop().await;
    second.stop().await;
    topic.delete().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_nodes_on_one_topic_deliver_every_event_to_the_subscribers_of_both() {
    let (topic, first, second) = cluster("two-nodes").await;
    let mut on_first = Sse::open(first.address, &subscription(TYPE), None).await;
    let mut on_second = Sse::open(second.address, &subscription(TYPE), None).await;

    topic.produce(&envelopes(1..=50)).await;
    let from_first = on_first.events(50).await;
    let from_second = on_second.events(50).await;
    let rests = [
        on_first.keep_alives(1).await,
        on_second.keep_alives(1).await,
    ];
    tear_down(topic, first, second).await;

    for rest in rests {
        assert!(rest.is_empty(), "{rest:?}");
    }
    let expected: Vec<u64> = (1..=50).collect();
    assert_eq!(ids(&from_first), expected);
    assert_eq!(ids(&from_second), expected);
    let produced: Vec<String> = (1..=50).map(envelope_id).collect();
    assert_eq!(envelope_ids(&from_first), produced);
    assert_eq!(envelope_ids(&from_second), produced);
    assert!(
        from_first
            .iter()
            .chain(&from_second)
            .all(|frame| frame.event == TYPE)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_resumes_on_the_other_node_with_the_last_id_it_received() {
    let (topic, first, second) = cluster("resume").await;
    let mut client = Sse::open(first.address, &subscription(TYPE), None).await;
    topic.produce(&envelopes(1..=6)).await;
    let before = client.events(6).await;
    drop(client);
    topic.produce(&envelopes(7..=10)).await;
    // The events up to 10 are retained on the second node before the client comes back.
    wait_for(second.address, "position", &json!("10")).await;

    let mut resumed = Sse::open(second.address, &subscription(TYPE), Some(6)).await;
    topic.produce(&envelopes(11..=12)).await;
    let after = resumed.events(6).await;
    let later = resumed.keep_alives(1).await;
    tear_down(topic, first, second).await;

    assert_eq!(ids(&before), [1, 2, 3, 4, 5, 6]);
    assert_eq!(ids(&after), [7, 8, 9, 10, 11, 12]);
    assert!(later.is_empty(), "every later event comes once: {later:?}");
    let expected: Vec<String> = (7..=12).map(envelope_id).collect();
    assert_eq!(envelope_ids(&after), expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_envelope_produced_twice_is_delivered_twice_with_one_envelope_id_and_two_ids() {
    let (topic, first, second) = cluster("duplicate").await;
    let mut on_first = Sse::open(first.address, &subscription(TYPE), None).await;
    let mut on_second = Sse::open(second.address, &subscription(TYPE), None).await;

    topic.produce(&[envelope(1), envelope(1)]).await;
    let from_first = on_first.events(2).await;
    let from_second = on_second.events(2).await;
    let rests = [
        on_first.keep_alives(1).await,
        on_second.keep_alives(1).await,
    ];
    tear_down(topic, first, second).await;

    for rest in rests {
        assert!(rest.is_empty(), "{rest:?}");
    }
    for frames in [&from_first, &from_second] {
        assert_eq!(ids(frames), [1, 2]);
        assert_eq!(envelope_ids(frames), [envelope_id(1), envelope_id(1)]);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stopped_node_changes_nothing_on_the_other() {
    let (mut topic, first, second) = cluster("stopped-node").await;
    let mut client = Sse::open(second.address, &subscription(TYPE), None).await;
    let before = status(second.address).await;
    topic.produce(&envelopes(1..=3)).await;
    let early = client.events(3).await;

    first.stop().await;
    topic.produce(&envelopes(4..=6)).await;
    let late = client.events(3).await;
    let later = client.keep_alives(1).await;
    let after = wait_for(second.address, "position", &json!("6")).await;
    second.stop().await;
    topic.delete().await;

    assert_eq!(ids(&early), [1, 2, 3]);
    assert_eq!(ids(&late), [4, 5, 6]);
    assert!(later.is_empty(), "{later:?}");
    assert_eq!(before["state"], "ready");
    assert_eq!(before["subscribers"], 1);
    for field in [
        "state",
        "reason",
        "brokerReachable",
        "lag",
        "subscribers",
        "rejectedRecords",
        "retentionLosses",
        "replayCapped",
    ] {
        assert_eq!(after[field], before[field], "{field}: {after}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_record_of_an_aborted_transaction_reaches_a_client() {
    let (topic, first, second) = cluster("aborted").await;
    let mut on_first = Sse::open(first.address, &subscription(TYPE), None).await;
    let mut on_second = Sse::open(second.address, &subscription(TYPE), None).await;

    topic.produce(&[envelope(1)]).await;
    topic
        .produce_in_transaction("aborted", &[envelope(2), envelope(3)], false)
        .await;
    topic.produce(&[envelope(4)]).await;
    let from_first = on_first.events(2).await;
    let from_second = on_second.events(2).await;
    let rests = [
        on_first.keep_alives(1).await,
        on_second.keep_alives(1).await,
    ];
    let statuses = [status(first.address).await, status(second.address).await];
    tear_down(topic, first, second).await;

    for rest in rests {
        assert!(rest.is_empty(), "{rest:?}");
    }
    for frames in [&from_first, &from_second] {
        assert_eq!(envelope_ids(frames), [envelope_id(1), envelope_id(4)]);
        // The aborted records took the offsets 1 and 2, the abort marker the offset 3, so the
        // fourth envelope is at offset 4: its cursor is 5.
        assert_eq!(ids(frames), [1, 5]);
    }
    for status in statuses {
        assert_eq!(status["rejectedRecords"], "0", "{status}");
    }
}
