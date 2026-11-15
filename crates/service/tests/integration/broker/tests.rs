//! The rewrite of the broker entries a forwarder relays, and the removal of a topic its test did
//! not delete.

use event_stream_domain::TopicProbe as _;
use event_stream_infrastructure::KafkaTopic;
use rstest::rstest;

use super::{POLL, Rewrite, TestTopic, settings};
use crate::limit::bounded;

/// The port of the forwarder in these examples.
const FORWARDER_PORT: u16 = 40_000;

/// The head of an answer up to its first broker entry: correlation id 7, one broker, node id 1.
const HEAD: [u8; 12] = [0, 0, 0, 7, 0, 0, 0, 1, 0, 0, 0, 1];

/// A broker entry with its host as a `STRING` (INT16 length).
fn string_entry(host: &str, port: i32) -> Vec<u8> {
    let length = u8::try_from(host.len()).unwrap();
    [&[0, length][..], host.as_bytes(), &port.to_be_bytes()].concat()
}

/// A broker entry with its host as a `COMPACT_STRING` (unsigned varint of the length plus 1).
fn compact_entry(host: &str, port: i32) -> Vec<u8> {
    let length = u8::try_from(host.len()).unwrap().saturating_add(1);
    [&[length][..], host.as_bytes(), &port.to_be_bytes()].concat()
}

#[test]
fn a_string_entry_of_a_broker_inside_the_compose_network_names_the_forwarder() {
    let rewrite = Rewrite::new("kafka", 9_092, FORWARDER_PORT);
    let frame = [&HEAD[..], &string_entry("kafka", 9_092), &[0xff, 0xff]].concat();

    let rewritten = rewrite.apply(&frame);

    let forwarder = string_entry("127.0.0.1", i32::from(FORWARDER_PORT));
    let expected = [&HEAD[..], &forwarder, &[0xff, 0xff]].concat();
    assert_eq!(rewritten, expected);
    assert_eq!(rewritten.len(), frame.len().saturating_add(4));
}

#[test]
fn every_compact_entry_of_a_broker_on_the_host_names_the_forwarder() {
    let rewrite = Rewrite::new("localhost", 9_095, FORWARDER_PORT);
    let entry = compact_entry("localhost", 9_095);
    let frame = [&HEAD[..], &entry, &[0, 2], &entry, &[0]].concat();

    let rewritten = rewrite.apply(&frame);

    let forwarder = compact_entry("127.0.0.1", i32::from(FORWARDER_PORT));
    let expected = [&HEAD[..], &forwarder, &[0, 2], &forwarder, &[0]].concat();
    assert_eq!(rewritten, expected);
}

#[test]
fn a_host_with_another_port_and_an_answer_without_a_broker_stay_unchanged() {
    let rewrite = Rewrite::new("kafka", 9_092, FORWARDER_PORT);
    let other_port = [&HEAD[..], &compact_entry("kafka", 9_093)].concat();

    assert_eq!(rewrite.apply(&other_port), other_port);
    assert_eq!(rewrite.apply(&HEAD), HEAD);
    assert_eq!(rewrite.apply(&[]), Vec::<u8>::new());
}

#[rstest]
#[case::no_prefix(vec![])]
#[case::another_length(vec![0, 4])]
#[case::a_string_of_more_than_255_bytes(vec![1, 5])]
#[should_panic(expected = "has a length prefix the forwarder cannot rewrite")]
fn an_entry_whose_prefix_is_not_the_length_of_the_host_fails(#[case] prefix: Vec<u8>) {
    let rewrite = Rewrite::new("kafka", 9_092, FORWARDER_PORT);
    let frame = [prefix, b"kafka".to_vec(), 9_092_i32.to_be_bytes().to_vec()].concat();

    rewrite.apply(&frame);
}

/// The partition count the broker gives for the topic `name` now, read through a probe of its own:
/// a probe keeps its reader, and the metadata that reader caches still names a removed topic until
/// it ages out.
async fn partition_count(name: &str) -> Option<u32> {
    let probe = KafkaTopic::new(settings(name)).probe();
    bounded(probe.partition_count()).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_topic_dropped_before_its_test_deleted_it_is_removed_from_the_broker() {
    let topic = TestTopic::create("dropped", 1).await;
    let name = topic.name.clone();
    let before = partition_count(&name).await;

    drop(topic);
    bounded(async {
        while partition_count(&name).await.is_some() {
            tokio::time::sleep(POLL).await;
        }
    })
    .await;

    assert_eq!(before, Some(1));
}
