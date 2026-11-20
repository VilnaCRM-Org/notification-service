//! GraphQL subscriptions over SSE against the broker of the stack (FR-10, FR-11, FR-12, FR-16,
//! FR-19 (b), NFR-5, NFR-18): served binaries on a topic of one partition of their own answer the
//! POST request of the graphql-sse client with subscription results, resume from the window, from
//! Kafka and on another node, end a stream at shutdown without `complete`, and count GraphQL
//! subscribers against `MAX_SUBSCRIBERS` together with the SSE and WebSocket subscribers.

use serde_json::{Value, json};

use crate::broker::{TYPE, TestTopic, bootstrap_servers, envelope, envelopes};
use crate::client::{
    Block, GraphqlSse, Sse, get, local, status, subscription, upgrade, upgrade_refused, wait_for,
};
use crate::support::{Answer, Scratch, Server, messages_of, serve};

/// The event type produced next to [`TYPE`], which no subscription of these tests lists.
const OTHER: &str = "order.paid";

/// The detail of the 503 problem of a node at its subscriber cap.
const AT_CAPACITY: &str = "the node is at its subscriber capacity";

/// The configuration of a served binary on `topic` through `bootstrap`: SQS on port 1, which no
/// request of these tests needs, one event type, short pauses after a failed round and a keep-alive
/// every second, which ends the read that shows that nothing follows the last expected block; then
/// `extra`.
fn variables<'a>(
    bootstrap: &'a str,
    topic: &'a str,
    extra: &[(&'a str, &'a str)],
) -> Vec<(&'a str, &'a str)> {
    let mut variables = vec![
        ("AWS_REGION", "us-east-1"),
        ("AWS_ENDPOINT_URL_SQS", "http://127.0.0.1:1"),
        ("KAFKA_BOOTSTRAP_SERVERS", bootstrap),
        ("KAFKA_TOPIC", topic),
        ("EVENT_TYPES", TYPE),
        ("KAFKA_RETRY_DELAY", "100ms"),
        ("STREAM_KEEP_ALIVE", "1s"),
    ];
    variables.extend_from_slice(extra);
    variables
}

/// A served binary on `topic` with [`variables`] and `extra`, once its event stream is ready.
async fn ready_node(scratch: &Scratch, topic: &str, extra: &[(&str, &str)]) -> Server {
    let bootstrap = bootstrap_servers();
    let server = serve(scratch, &variables(&bootstrap, topic, extra));
    wait_for(local(server.port), "state", &json!("ready")).await;
    server
}

/// The request of a subscription of [`TYPE`] that selects every field of `Event`, with the
/// argument `lastEventId` when `last_event_id` is given.
fn subscribe(last_event_id: Option<&str>) -> Value {
    let cursor = last_event_id.map_or_else(String::new, |id| format!(r#", lastEventId: "{id}""#));
    json!({
        "query": format!(r#"subscription {{ events(types: ["{TYPE}"]{cursor}) {{ id type data }} }}"#)
    })
}

/// A valid envelope of [`OTHER`] numbered `number`.
fn other_envelope(number: u64) -> Vec<u8> {
    String::from_utf8(envelope(number))
        .unwrap()
        .replace(TYPE, OTHER)
        .into_bytes()
}

/// The subscription results of the envelopes of [`TYPE`] numbered `numbers`, each produced as the
/// record of id `number`: a `next` block with that event id and the event object the producer
/// wrote.
fn results(numbers: impl IntoIterator<Item = u64>) -> Vec<Block> {
    numbers
        .into_iter()
        .map(|number| {
            let object: Value = serde_json::from_slice(&envelope(number)).unwrap();
            Block::Next {
                id: Some(number),
                result: json!({
                    "data": {"events": {"id": number.to_string(), "type": TYPE, "data": object}}
                }),
            }
        })
        .collect()
}

/// The event ids of `blocks`, each of which must be a subscription result.
fn event_ids(blocks: &[Block]) -> Vec<u64> {
    blocks
        .iter()
        .map(|block| match block {
            Block::Next { id: Some(id), .. } => *id,
            Block::Next { id: None, .. } | Block::Complete => {
                panic!("{block:?} is not a subscription result")
            }
        })
        .collect()
}

/// Asserts the 503 problem of a node at its subscriber cap.
fn assert_at_capacity(answer: &Answer) {
    assert_eq!(answer.status, 503, "{}", answer.body);
    assert!(
        answer
            .head
            .contains("content-type: application/problem+json"),
        "{}",
        answer.head
    );
    let problem: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(problem["status"], 503, "{problem}");
    assert_eq!(problem["detail"], AT_CAPACITY, "{problem}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_subscription_receives_each_event_of_its_type_and_none_of_another_type() {
    let mut topic = TestTopic::create("graphql-delivery", 1).await;
    let scratch = Scratch::new();
    let server = ready_node(&scratch, &topic.name, &[]).await;
    let mut client = GraphqlSse::open(local(server.port), &subscribe(None), None).await;

    topic.produce(&[other_envelope(1), envelope(2)]).await;
    let delivered = client.blocks(1).await;
    let rest = client.comments(1).await;
    let finished = server.terminate();
    topic.delete().await;
    drop(topic);

    assert_eq!(event_ids(&delivered), [2]);
    assert_eq!(delivered, results([2]));
    assert!(
        rest.is_empty(),
        "nothing follows the event of type a: {rest:?}"
    );
    assert_eq!(finished.code, Some(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_resume_by_argument_by_header_or_by_both_starts_above_the_cursor_that_applies() {
    let mut topic = TestTopic::create("graphql-resume", 1).await;
    let scratch = Scratch::new();
    let server = ready_node(&scratch, &topic.name, &[]).await;
    let address = local(server.port);
    let mut first = GraphqlSse::open(address, &subscribe(None), None).await;
    topic
        .produce(&[envelope(1), other_envelope(2), envelope(3)])
        .await;
    let before = first.blocks(2).await;
    drop(first);
    let away = [
        other_envelope(4),
        envelope(5),
        envelope(6),
        other_envelope(7),
        envelope(8),
    ];
    topic.produce(&away).await;
    // The events up to 8 are retained before the client comes back.
    wait_for(address, "position", &json!("8")).await;

    let mut by_argument = GraphqlSse::open(address, &subscribe(Some("3")), None).await;
    let mut by_header = GraphqlSse::open(address, &subscribe(None), Some(3)).await;
    // The header holds a later id than the argument, and wins.
    let mut by_both = GraphqlSse::open(address, &subscribe(Some("3")), Some(6)).await;
    topic.produce(&[envelope(9)]).await;
    let argument = by_argument.blocks(4).await;
    let header = by_header.blocks(4).await;
    let both = by_both.blocks(2).await;
    let rests = [
        by_argument.comments(1).await,
        by_header.comments(1).await,
        by_both.comments(1).await,
    ];
    let finished = server.terminate();
    topic.delete().await;
    drop(topic);

    assert_eq!(before, results([1, 3]));
    assert_eq!(event_ids(&argument), [5, 6, 8, 9]);
    assert_eq!(argument, results([5, 6, 8, 9]));
    assert_eq!(event_ids(&header), [5, 6, 8, 9]);
    assert_eq!(header, results([5, 6, 8, 9]));
    assert_eq!(event_ids(&both), [8, 9]);
    assert_eq!(both, results([8, 9]));
    for rest in rests {
        assert!(rest.is_empty(), "every later event comes once: {rest:?}");
    }
    assert_eq!(finished.code, Some(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_resume_below_the_window_reads_kafka_then_the_window_then_live_events() {
    let mut topic = TestTopic::create("graphql-replay", 1).await;
    let scratch = Scratch::new();
    let window = [("EVENT_WINDOW_MAX_EVENTS", "10")];
    let server = ready_node(&scratch, &topic.name, &window).await;
    let address = local(server.port);
    topic.produce(&envelopes(1..=25)).await;
    let held = wait_for(address, "position", &json!("25")).await;

    let mut client = GraphqlSse::open(address, &subscribe(Some("3")), None).await;
    topic.produce(&envelopes(26..=27)).await;
    let resumed = client.blocks(24).await;
    let rest = client.comments(1).await;
    let finished = server.terminate();
    topic.delete().await;
    drop(topic);

    let floor: u64 = held["windowFirst"].as_str().unwrap().parse().unwrap();
    assert!(floor > 3, "the cursor 3 lies below the window: {held}");
    let expected: Vec<u64> = (4..=27).collect();
    assert_eq!(event_ids(&resumed), expected, "no gap and no duplicate");
    assert_eq!(resumed, results(4..=27));
    assert!(rest.is_empty(), "every later event comes once: {rest:?}");
    assert_eq!(finished.code, Some(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_resumes_on_another_node_of_the_topic_with_the_last_id_it_received() {
    let mut topic = TestTopic::create("graphql-other-node", 1).await;
    let scratch = Scratch::new();
    let first = ready_node(&scratch, &topic.name, &[]).await;
    let second = ready_node(&scratch, &topic.name, &[]).await;
    let mut client = GraphqlSse::open(local(first.port), &subscribe(None), None).await;
    topic.produce(&envelopes(1..=4)).await;
    let before = client.blocks(4).await;
    drop(client);
    topic.produce(&envelopes(5..=7)).await;
    // The events up to 7 are retained on the second node before the client comes back.
    wait_for(local(second.port), "position", &json!("7")).await;

    let mut resumed = GraphqlSse::open(local(second.port), &subscribe(None), Some(4)).await;
    topic.produce(&envelopes(8..=8)).await;
    let after = resumed.blocks(4).await;
    let rest = resumed.comments(1).await;
    let finished = [first.terminate(), second.terminate()];
    topic.delete().await;
    drop(topic);

    assert_eq!(before, results(1..=4));
    assert_eq!(event_ids(&after), [5, 6, 7, 8]);
    assert_eq!(after, results(5..=8));
    assert!(rest.is_empty(), "every later event comes once: {rest:?}");
    for node in finished {
        assert_eq!(node.code, Some(0));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_shutdown_ends_the_stream_after_its_queued_results_without_complete() {
    let mut topic = TestTopic::create("graphql-shutdown", 1).await;
    let scratch = Scratch::new();
    let stopping = ready_node(&scratch, &topic.name, &[]).await;
    let running = ready_node(&scratch, &topic.name, &[]).await;
    let client = GraphqlSse::open(local(stopping.port), &subscribe(None), None).await;
    topic.produce(&envelopes(1..=3)).await;
    // The tail hands an event to the subscribers before its position passes it, so the three
    // events are queued for the client once the position is 3; the client reads none of them yet.
    wait_for(local(stopping.port), "position", &json!("3")).await;

    stopping.signal("TERM");
    let delivered = client.rest().await;
    let stopped = stopping.finish();
    topic.produce(&envelopes(4..=5)).await;
    wait_for(local(running.port), "position", &json!("5")).await;
    let mut resumed = GraphqlSse::open(local(running.port), &subscribe(None), Some(3)).await;
    topic.produce(&envelopes(6..=6)).await;
    let after = resumed.blocks(3).await;
    let rest = resumed.comments(1).await;
    let finished = running.terminate();
    topic.delete().await;
    drop(topic);

    assert_eq!(event_ids(&delivered), [1, 2, 3]);
    assert_eq!(
        delivered,
        results(1..=3),
        "the queued results, then the end without complete"
    );
    assert_eq!(stopped.code, Some(0));
    assert_eq!(
        messages_of(&stopped.lines, "shutdown signal received").len(),
        1
    );
    assert_eq!(after, results(4..=6), "the resume misses no retained event");
    assert!(rest.is_empty(), "every later event comes once: {rest:?}");
    assert_eq!(finished.code, Some(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn graphql_subscribers_count_against_the_cap_with_the_sse_and_websocket_subscribers() {
    let mut topic = TestTopic::create("graphql-cap", 1).await;
    let scratch = Scratch::new();
    let server = ready_node(&scratch, &topic.name, &[("MAX_SUBSCRIBERS", "3")]).await;
    let address = local(server.port);
    let sse = Sse::open(address, &subscription(TYPE), None).await;
    let socket = upgrade(address, TYPE).await;
    let graphql = GraphqlSse::open(address, &subscribe(None), None).await;
    let full = status(address).await;

    let refused = [
        get(
            address,
            &format!("/api/events/stream{}", subscription(TYPE)),
        )
        .await,
        upgrade_refused(address, TYPE).await,
        GraphqlSse::refused(address, &subscribe(None), None).await,
    ];
    let held = status(address).await;
    drop(graphql);
    let freed = wait_for(address, "subscribers", &json!(2)).await;
    let reopened = GraphqlSse::open(address, &subscribe(None), None).await;
    let refilled = status(address).await;
    drop((sse, socket, reopened));
    let finished = server.terminate();
    topic.delete().await;
    drop(topic);

    assert_eq!(full["subscribers"], 3, "{full}");
    for answer in &refused {
        assert_at_capacity(answer);
    }
    let [_, _, graphql_refusal] = &refused;
    assert!(
        !graphql_refusal.head.contains("text/event-stream"),
        "the refusal comes before any event-stream head: {}",
        graphql_refusal.head
    );
    assert_eq!(held["subscribers"], 3, "{held}");
    assert_eq!(freed["subscribers"], 2, "{freed}");
    assert_eq!(refilled["subscribers"], 3, "{refilled}");
    let rejected = messages_of(&finished.lines, "subscription rejected");
    assert_eq!(rejected.len(), 3, "{rejected:?}");
    for line in rejected {
        assert_eq!(line["level"], "WARN", "{line}");
        assert_eq!(line["detail"], AT_CAPACITY, "{line}");
    }
}
