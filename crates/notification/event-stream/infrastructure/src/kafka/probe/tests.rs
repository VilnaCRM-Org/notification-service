use std::time::Duration;

use event_stream_domain::{Cursor, TopicProbe as _, Watermarks};
use krafka::error::ErrorCode;
use krafka::testing::{ApiKey, Control};
use tokio::time::Instant;

use crate::kafka::tests::{
    Broker, REQUEST_TIMEOUT, TOPIC, bounded_test, settings, unavailable_source,
};
use crate::kafka::{KafkaSecurity, KafkaSettings};

/// Longer than the client's metadata max age of 300 s (`krafka` `client.rs:82`) and shorter than its
/// idle-connection time-out of 540 s: after it the next partition count asks the broker again
/// instead of answering from the client's cache.
const METADATA_STALE_AFTER: Duration = Duration::from_secs(301);

#[tokio::test(start_paused = true)]
async fn the_partition_count_of_a_topic_of_one_partition_is_one() {
    let broker = Broker::start();
    let probe = broker.topic().probe();

    assert_eq!(
        bounded_test(probe.partition_count()).await.unwrap(),
        Some(1)
    );
}

#[tokio::test(start_paused = true)]
async fn the_partition_count_of_a_topic_of_three_partitions_is_three() {
    let broker = Broker::start();
    assert!(broker.fake.create_topic("wide", 3));
    let settings = KafkaSettings {
        topic: "wide".to_owned(),
        ..settings(KafkaSecurity::Plaintext)
    };
    let probe = broker.topic_with(settings).probe();

    assert_eq!(
        bounded_test(probe.partition_count()).await.unwrap(),
        Some(3)
    );
}

#[tokio::test(start_paused = true)]
async fn the_watermarks_of_an_empty_topic_are_equal() {
    let broker = Broker::start();
    let probe = broker.topic().probe();

    let watermarks = bounded_test(probe.watermarks()).await.unwrap();
    drop(probe);

    assert_eq!(
        watermarks,
        Watermarks {
            earliest: Cursor::START,
            end: Cursor::START,
        }
    );
}

#[tokio::test(start_paused = true)]
async fn the_watermarks_are_the_log_start_and_the_end_the_broker_reports() {
    let broker = Broker::start();
    broker.produce(&["a", "b", "c", "d"]).await;
    broker.move_log_start(2);
    let probe = broker.topic().probe();

    let watermarks = bounded_test(probe.watermarks()).await.unwrap();
    drop(probe);

    assert_eq!(
        watermarks,
        Watermarks {
            earliest: Cursor::new(2),
            end: Cursor::new(4),
        }
    );
}

#[tokio::test(start_paused = true)]
async fn the_end_is_the_last_stable_offset_below_an_open_transaction() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    broker.fake.hold_transaction_markers(true);
    broker.produce_in_transaction("open", &["b"], true).await;
    let probe = broker.topic().probe();

    let watermarks = bounded_test(probe.watermarks()).await.unwrap();
    drop(probe);

    assert_eq!(watermarks.end, Cursor::new(1));
}

#[tokio::test(start_paused = true)]
async fn the_probe_keeps_its_reader_between_calls() {
    let broker = Broker::start();
    let probe = broker.topic().probe();

    bounded_test(probe.partition_count()).await.unwrap();
    bounded_test(probe.watermarks()).await.unwrap();
    bounded_test(probe.watermarks()).await.unwrap();
    drop(probe);

    assert_eq!(broker.connections(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_silent_broker_makes_the_watermarks_unavailable_within_the_request_time_out() {
    let broker = Broker::start();
    let probe = broker.topic().probe();
    bounded_test(probe.watermarks()).await.unwrap();
    broker.fake.on(ApiKey::ListOffsets, |_| Control::Silence);
    let started = Instant::now();

    let result = bounded_test(probe.watermarks()).await;
    let elapsed = started.elapsed();
    drop(probe);

    unavailable_source(result);
    assert_eq!(elapsed, REQUEST_TIMEOUT);
}

#[tokio::test(start_paused = true)]
async fn a_dropped_connection_is_unavailable_and_the_next_call_connects_again() {
    let broker = Broker::start();
    let probe = broker.topic().probe();
    bounded_test(probe.watermarks()).await.unwrap();
    broker
        .fake
        .on_once(ApiKey::ListOffsets, |_| Control::Disconnect);

    let failed = bounded_test(probe.watermarks()).await;
    let connections_after_failure = broker.connections();
    let recovered = bounded_test(probe.watermarks()).await;
    drop(probe);

    unavailable_source(failed);
    assert_eq!(connections_after_failure, 1);
    assert_eq!(recovered.unwrap().end, Cursor::START);
    assert_eq!(broker.connections(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_silent_broker_makes_the_partition_count_unavailable_within_the_request_time_out() {
    let broker = Broker::start();
    let probe = broker.topic().probe();
    bounded_test(probe.partition_count()).await.unwrap();
    tokio::time::advance(METADATA_STALE_AFTER).await;
    broker.fake.on(ApiKey::Metadata, |_| Control::Silence);
    let started = Instant::now();

    let silent = bounded_test(probe.partition_count()).await;
    let elapsed = started.elapsed();
    let idle_after_failure = probe.pool.locked().len();
    broker.fake.clear_hooks();
    let recovered = bounded_test(probe.partition_count()).await;
    drop(probe);

    unavailable_source(silent);
    assert_eq!(elapsed, REQUEST_TIMEOUT);
    assert_eq!(idle_after_failure, 0);
    assert_eq!(recovered.unwrap(), Some(1));
    assert_eq!(broker.connections(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_dropped_connection_makes_the_partition_count_unavailable_and_the_next_call_connects_again()
 {
    let broker = Broker::start();
    let probe = broker.topic().probe();
    bounded_test(probe.partition_count()).await.unwrap();
    tokio::time::advance(METADATA_STALE_AFTER).await;
    broker
        .fake
        .on_once(ApiKey::Metadata, |_| Control::Disconnect);

    let failed = bounded_test(probe.partition_count()).await;
    let idle_after_failure = probe.pool.locked().len();
    let recovered = bounded_test(probe.partition_count()).await;
    drop(probe);

    unavailable_source(failed);
    assert_eq!(idle_after_failure, 0);
    assert_eq!(recovered.unwrap(), Some(1));
    assert_eq!(broker.connections(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_broker_error_on_the_watermarks_is_unavailable() {
    let broker = Broker::start();
    broker.fake.on(ApiKey::ListOffsets, |_| {
        Control::Error(ErrorCode::UnknownServerError)
    });
    let probe = broker.topic().probe();

    let source = unavailable_source(bounded_test(probe.watermarks()).await);
    drop(probe);

    assert!(source.contains("UnknownServerError"), "{source}");
}

#[tokio::test(start_paused = true)]
async fn a_crashed_broker_is_unavailable_and_a_restarted_one_answers_again() {
    let broker = Broker::start();
    let probe = broker.topic().probe();
    broker.fake.crash(0);

    let crashed = bounded_test(probe.partition_count()).await;
    broker.fake.restart(0);
    let restarted = bounded_test(probe.partition_count()).await;
    drop(probe);

    unavailable_source(crashed);
    assert_eq!(restarted.unwrap(), Some(1));
}

#[tokio::test(start_paused = true)]
async fn a_crash_after_a_successful_call_is_unavailable_and_the_probe_recovers_after_the_restart() {
    let broker = Broker::start();
    let probe = broker.topic().probe();
    bounded_test(probe.watermarks()).await.unwrap();
    broker.fake.crash(0);

    let crashed = bounded_test(probe.watermarks()).await;
    broker.fake.restart(0);
    let restarted = bounded_test(probe.watermarks()).await;
    drop(probe);

    unavailable_source(crashed);
    assert_eq!(restarted.unwrap().end, Cursor::START);
    assert!(broker.connections() >= 2);
    assert_eq!(broker.requests_of_readers(ApiKey::CreateTopics), 0);
    assert!(
        broker
            .fake
            .with_state(|state| state.partition(TOPIC, 0).is_some())
    );
}
