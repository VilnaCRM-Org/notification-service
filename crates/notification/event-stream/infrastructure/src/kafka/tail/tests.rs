use std::time::Duration;

use event_stream_domain::{Batch, Cursor, Offset, TopicError, TopicRecord, TopicTail as _};
use krafka::error::ErrorCode;
use krafka::testing::{ApiKey, Control};
use tokio::time::Instant;

use crate::kafka::tests::{
    Broker, FETCH_WAIT, REQUEST_TIMEOUT, TOPIC, bounded_test, record, unavailable_source,
};

/// How long the broker holds back an answer that must arrive after the wait and before `t`.
const ANSWER_DELAY: Duration = Duration::from_secs(2);

fn empty_batch(next: u64) -> Batch {
    Batch {
        records: Vec::new(),
        next: Cursor::new(next),
    }
}

#[tokio::test(start_paused = true)]
async fn a_fetch_after_a_seek_returns_the_records_from_that_position_and_moves_on() {
    let broker = Broker::start();
    broker.produce(&["a", "b", "c", "d"]).await;
    let mut tail = broker.topic().tail();

    bounded_test(tail.seek(Cursor::new(2))).await.unwrap();
    let batch = bounded_test(tail.fetch()).await.unwrap();

    assert_eq!(
        batch,
        Batch {
            records: vec![record(2, "c"), record(3, "d")],
            next: Cursor::new(4),
        }
    );
    assert_eq!(tail.position, Cursor::new(4));
}

#[tokio::test(start_paused = true)]
async fn a_fetch_without_a_seek_reads_from_the_start() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    let mut tail = broker.topic().tail();

    let batch = bounded_test(tail.fetch()).await.unwrap();
    drop(tail);

    assert_eq!(batch.records, vec![record(0, "a")]);
    assert_eq!(batch.next, Cursor::new(1));
}

#[tokio::test(start_paused = true)]
async fn the_next_fetch_continues_after_the_last_one_and_long_polls_for_new_records() {
    let broker = Broker::start();
    broker.produce(&["a", "b"]).await;
    let mut tail = broker.topic().tail();
    bounded_test(tail.fetch()).await.unwrap();
    let fetches_before = broker.requests_of_readers(ApiKey::Fetch);
    let started = Instant::now();

    let empty = bounded_test(tail.fetch()).await.unwrap();
    let waited = started.elapsed();
    let fetches_of_the_long_poll = broker.requests_of_readers(ApiKey::Fetch) - fetches_before;
    broker.produce(&["c"]).await;
    let next = bounded_test(tail.fetch()).await.unwrap();
    drop(tail);

    assert_eq!(empty, empty_batch(2));
    assert_eq!(waited, FETCH_WAIT);
    assert_eq!(fetches_of_the_long_poll, 1);
    assert_eq!(next.records, vec![record(2, "c")]);
    assert_eq!(next.next, Cursor::new(3));
    assert_eq!(broker.connections(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_seek_moves_the_reader_back_to_records_it_already_read() {
    let broker = Broker::start();
    broker.produce(&["a", "b", "c"]).await;
    let mut tail = broker.topic().tail();
    bounded_test(tail.fetch()).await.unwrap();

    bounded_test(tail.seek(Cursor::new(1))).await.unwrap();
    let batch = bounded_test(tail.fetch()).await.unwrap();
    drop(tail);

    assert_eq!(batch.records, vec![record(1, "b"), record(2, "c")]);
    assert_eq!(broker.connections(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_record_without_a_value_is_handed_out_without_one() {
    let broker = Broker::start();
    broker.produce_tombstone().await;
    let mut tail = broker.topic().tail();

    let batch = bounded_test(tail.fetch()).await.unwrap();
    drop(tail);

    assert_eq!(
        batch.records,
        vec![TopicRecord {
            offset: Offset::new(0),
            value: None,
        }]
    );
}

#[tokio::test(start_paused = true)]
async fn records_of_an_aborted_transaction_are_never_returned_and_the_position_passes_the_markers()
{
    let broker = Broker::start();
    broker.produce(&["before"]).await;
    broker
        .produce_in_transaction("aborted", &["lost-1", "lost-2"], false)
        .await;
    broker
        .produce_in_transaction("committed", &["kept"], true)
        .await;
    let end = broker.fake.next_offset(TOPIC, 0).unwrap();
    let mut tail = broker.topic().tail();

    let mut records = Vec::new();
    let mut next = Cursor::START;
    for _ in 0..3 {
        let batch = bounded_test(tail.fetch()).await.unwrap();
        records.extend(batch.records);
        next = batch.next;
    }
    drop(tail);

    let values: Vec<_> = records
        .iter()
        .map(|record| String::from_utf8(record.value.clone().unwrap()).unwrap())
        .collect();
    assert_eq!(values, vec!["before", "kept"]);
    assert_eq!(next, Cursor::new(u64::try_from(end).unwrap()));
    assert!(next > Cursor::new(4), "{next}");
}

#[tokio::test(start_paused = true)]
async fn a_position_the_broker_has_no_offset_for_is_below_earliest_with_the_log_start() {
    let broker = Broker::start();
    broker.produce(&["a", "b", "c"]).await;
    broker.move_log_start(1);
    let mut tail = broker.topic().tail();

    bounded_test(tail.seek(Cursor::new(9))).await.unwrap();
    let result = bounded_test(tail.fetch()).await;
    let kept_reader = tail.reader.is_some();
    let position = tail.position;
    bounded_test(tail.seek(Cursor::new(1))).await.unwrap();
    let batch = bounded_test(tail.fetch()).await.unwrap();
    drop(tail);

    assert!(
        matches!(result, Err(TopicError::BelowEarliest { earliest }) if earliest == Cursor::new(1)),
        "{result:?}"
    );
    assert!(kept_reader);
    assert_eq!(position, Cursor::new(9));
    assert_eq!(batch.records, vec![record(1, "b"), record(2, "c")]);
    assert_eq!(broker.connections(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_failed_read_of_the_log_start_is_unavailable_and_drops_the_reader() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    let mut tail = broker.topic().tail();
    bounded_test(tail.seek(Cursor::new(9))).await.unwrap();
    broker.fake.on(ApiKey::ListOffsets, |_| {
        Control::Error(ErrorCode::UnknownServerError)
    });

    let result = bounded_test(tail.fetch()).await;
    let kept_reader = tail.reader.is_some();
    drop(tail);

    assert!(unavailable_source(result).contains("UnknownServerError"));
    assert!(!kept_reader);
}

#[tokio::test(start_paused = true)]
async fn a_fetch_below_earliest_whose_log_start_read_hangs_ends_at_the_limit_of_the_fetch() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    let mut tail = broker.topic().tail();
    bounded_test(tail.seek(Cursor::new(9))).await.unwrap();
    broker
        .fake
        .on_once(ApiKey::Fetch, |_| Control::Delay(ANSWER_DELAY));
    broker.fake.on(ApiKey::ListOffsets, |_| Control::Silence);
    let started = Instant::now();

    let result = bounded_test(tail.fetch()).await;
    let elapsed = started.elapsed();
    let kept_reader = tail.reader.is_some();
    drop(tail);

    assert_eq!(unavailable_source(result), "deadline has elapsed");
    assert_eq!(elapsed, REQUEST_TIMEOUT + FETCH_WAIT);
    assert!(!kept_reader);
}

#[tokio::test(start_paused = true)]
async fn corrupt_records_are_unavailable_and_nothing_is_skipped() {
    let broker = Broker::start();
    broker.produce(&["a", "b"]).await;
    let mut tail = broker.topic().tail();
    broker
        .fake
        .on_once(ApiKey::Fetch, |_| Control::CorruptRecords);

    let corrupt = bounded_test(tail.fetch()).await;
    let position_after_failure = tail.position;
    let reader_after_failure = tail.reader.is_some();
    let recovered = bounded_test(tail.fetch()).await.unwrap();
    drop(tail);

    assert!(unavailable_source(corrupt).contains("protocol error"));
    assert_eq!(position_after_failure, Cursor::START);
    assert!(!reader_after_failure);
    assert_eq!(recovered.records, vec![record(0, "a"), record(1, "b")]);
    assert_eq!(broker.connections(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_silent_broker_ends_a_fetch_empty_at_the_request_time_out() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    let mut tail = broker.topic().tail();
    bounded_test(tail.fetch()).await.unwrap();
    broker.fake.on(ApiKey::Fetch, |_| Control::Silence);
    let started = Instant::now();

    let result = bounded_test(tail.fetch()).await;

    assert_eq!(started.elapsed(), REQUEST_TIMEOUT);
    assert_eq!(result.unwrap(), empty_batch(1));
    assert_eq!(tail.position, Cursor::new(1));
}

#[tokio::test(start_paused = true)]
async fn an_answer_that_comes_too_late_ends_the_fetch_within_its_limit_and_the_next_one_reads_on() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    let mut tail = broker.topic().tail();
    bounded_test(tail.fetch()).await.unwrap();
    broker.fake.on_once(ApiKey::Fetch, |_| {
        Control::Delay(REQUEST_TIMEOUT + FETCH_WAIT + Duration::from_secs(1))
    });
    let started = Instant::now();

    let late = bounded_test(tail.fetch()).await;
    let elapsed = started.elapsed();
    broker.produce(&["b"]).await;
    let recovered = bounded_test(tail.fetch()).await.unwrap();
    drop(tail);

    assert_eq!(late.unwrap(), empty_batch(1));
    assert_eq!(elapsed, REQUEST_TIMEOUT);
    assert_eq!(recovered.records, vec![record(1, "b")]);
}

#[tokio::test(start_paused = true)]
async fn a_seek_on_a_crashed_broker_is_unavailable_and_the_fetch_after_the_restart_reads_from_its_position()
 {
    let broker = Broker::start();
    broker.produce(&["a", "b", "c"]).await;
    let mut tail = broker.topic().tail();
    broker.fake.crash(0);

    let crashed = bounded_test(tail.seek(Cursor::new(2))).await;
    broker.fake.restart(0);
    let batch = bounded_test(tail.fetch()).await.unwrap();
    drop(tail);

    unavailable_source(crashed);
    assert_eq!(batch.records, vec![record(2, "c")]);
}

#[tokio::test(start_paused = true)]
async fn a_dropped_connection_during_a_fetch_loses_no_record_and_repeats_none() {
    let broker = Broker::start();
    broker.produce(&["a", "b"]).await;
    let mut tail = broker.topic().tail();
    bounded_test(tail.seek(Cursor::new(1))).await.unwrap();
    broker.fake.on_once(ApiKey::Fetch, |_| Control::Disconnect);

    let mut records = Vec::new();
    for _ in 0..3 {
        if let Ok(batch) = bounded_test(tail.fetch()).await {
            records.extend(batch.records);
        }
    }

    assert_eq!(records, vec![record(1, "b")]);
    assert_eq!(tail.position, Cursor::new(2));
}
