use std::time::Duration;

use event_stream_domain::{Batch, Cursor, TopicError, TopicReplay as _};
use krafka::testing::{ApiKey, Control};
use tokio::time::Instant;

use super::REPLAY_WAIT;
use crate::kafka::tests::{Broker, REQUEST_TIMEOUT, bounded_test, record, unavailable_source};

#[test]
fn a_page_waits_a_tenth_of_a_second() {
    assert_eq!(REPLAY_WAIT, Duration::from_millis(100));
}

#[tokio::test(start_paused = true)]
async fn a_page_holds_the_records_from_the_requested_position_and_the_position_after_them() {
    let broker = Broker::start();
    broker.produce(&["a", "b", "c", "d"]).await;
    let replay = broker.topic().replay();

    let page = bounded_test(replay.page(Cursor::new(1))).await.unwrap();
    drop(replay);

    assert_eq!(
        page,
        Batch {
            records: vec![record(1, "b"), record(2, "c"), record(3, "d")],
            next: Cursor::new(4),
        }
    );
}

#[tokio::test(start_paused = true)]
async fn a_page_at_the_end_is_empty_after_its_short_wait() {
    let broker = Broker::start();
    broker.produce(&["a", "b"]).await;
    let replay = broker.topic().replay();
    bounded_test(replay.page(Cursor::START)).await.unwrap();
    let started = Instant::now();

    let page = bounded_test(replay.page(Cursor::new(2))).await.unwrap();
    let elapsed = started.elapsed();
    drop(replay);

    assert_eq!(
        page,
        Batch {
            records: Vec::new(),
            next: Cursor::new(2),
        }
    );
    assert_eq!(elapsed, REPLAY_WAIT);
}

#[tokio::test(start_paused = true)]
async fn a_pooled_reader_that_read_a_higher_page_starts_at_the_lower_position() {
    let broker = Broker::start();
    broker.produce(&["a", "b", "c", "d", "e"]).await;
    let replay = broker.topic().replay();

    let high = bounded_test(replay.page(Cursor::new(3))).await.unwrap();
    let low = bounded_test(replay.page(Cursor::new(1))).await.unwrap();
    let idle = replay.pool.locked().len();
    drop(replay);

    assert_eq!(high.records, vec![record(3, "d"), record(4, "e")]);
    assert_eq!(
        low,
        Batch {
            records: vec![
                record(1, "b"),
                record(2, "c"),
                record(3, "d"),
                record(4, "e")
            ],
            next: Cursor::new(5),
        }
    );
    assert_eq!(idle, 1);
    assert_eq!(broker.connections(), 1);
}

#[tokio::test(start_paused = true)]
async fn concurrent_pages_each_take_a_reader_of_their_own_and_give_it_back() {
    let broker = Broker::start();
    broker.produce(&["a", "b"]).await;
    let replay = broker.topic().replay();
    broker.fake.on_times(ApiKey::Fetch, 2, |_| {
        Control::Delay(Duration::from_millis(50))
    });

    let (first, second) = bounded_test(async {
        tokio::join!(replay.page(Cursor::START), replay.page(Cursor::new(1)))
    })
    .await;
    let idle = replay.pool.locked().len();
    drop(replay);

    assert_eq!(first.unwrap().records, vec![record(0, "a"), record(1, "b")]);
    assert_eq!(second.unwrap().records, vec![record(1, "b")]);
    assert_eq!(idle, 2);
    assert_eq!(broker.connections(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_failed_page_drops_its_reader_and_the_next_page_connects_again() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    let replay = broker.topic().replay();
    bounded_test(replay.page(Cursor::START)).await.unwrap();
    broker
        .fake
        .on_once(ApiKey::Fetch, |_| Control::CorruptRecords);

    let failed = bounded_test(replay.page(Cursor::START)).await;
    let idle_after_failure = replay.pool.locked().len();
    let recovered = bounded_test(replay.page(Cursor::START)).await.unwrap();
    let idle_after_recovery = replay.pool.locked().len();
    drop(replay);

    assert!(unavailable_source(failed).contains("protocol error"));
    assert_eq!(idle_after_failure, 0);
    assert_eq!(recovered.records, vec![record(0, "a")]);
    assert_eq!(idle_after_recovery, 1);
    assert_eq!(broker.connections(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_silent_broker_ends_a_page_empty_at_the_request_time_out() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    let replay = broker.topic().replay();
    bounded_test(replay.page(Cursor::START)).await.unwrap();
    broker.fake.on(ApiKey::Fetch, |_| Control::Silence);
    let started = Instant::now();

    let result = bounded_test(replay.page(Cursor::START)).await;
    let elapsed = started.elapsed();
    drop(replay);

    assert_eq!(elapsed, REQUEST_TIMEOUT);
    assert_eq!(
        result.unwrap(),
        Batch {
            records: Vec::new(),
            next: Cursor::START,
        }
    );
}

#[tokio::test(start_paused = true)]
async fn a_page_from_a_position_the_broker_has_no_offset_for_is_below_earliest_and_drops_its_reader()
 {
    let broker = Broker::start();
    broker.produce(&["a", "b"]).await;
    broker.move_log_start(1);
    let replay = broker.topic().replay();

    let result = bounded_test(replay.page(Cursor::new(7))).await;
    let idle = replay.pool.locked().len();
    let next_page = bounded_test(replay.page(Cursor::new(1))).await.unwrap();
    drop(replay);

    assert!(
        matches!(result, Err(TopicError::BelowEarliest { earliest }) if earliest == Cursor::new(1)),
        "{result:?}"
    );
    assert_eq!(idle, 0);
    assert_eq!(next_page.records, vec![record(1, "b")]);
    assert_eq!(broker.connections(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_page_below_earliest_whose_log_start_read_hangs_ends_at_the_limit_of_the_page() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    let replay = broker.topic().replay();
    broker
        .fake
        .on_once(ApiKey::Fetch, |_| Control::Delay(Duration::from_secs(2)));
    broker.fake.on(ApiKey::ListOffsets, |_| Control::Silence);
    let started = Instant::now();

    let result = bounded_test(replay.page(Cursor::new(9))).await;
    let elapsed = started.elapsed();
    let idle = replay.pool.locked().len();
    drop(replay);

    assert_eq!(unavailable_source(result), "deadline has elapsed");
    assert_eq!(elapsed, REQUEST_TIMEOUT + REPLAY_WAIT);
    assert_eq!(idle, 0);
}

#[tokio::test(start_paused = true)]
async fn a_page_on_a_crashed_broker_is_unavailable_at_once() {
    let broker = Broker::start();
    broker.fake.crash(0);
    let replay = broker.topic().replay();
    let started = Instant::now();

    let result = bounded_test(replay.page(Cursor::START)).await;
    let elapsed = started.elapsed();
    let idle = replay.pool.locked().len();
    drop(replay);

    unavailable_source(result);
    assert_eq!(elapsed, Duration::ZERO);
    assert_eq!(idle, 0);
}
