use std::time::Duration;

use event_stream_domain::{
    Batch, Cursor, Offset, TopicError, TopicProbe as _, TopicReplay as _, TopicTail as _,
    Watermarks,
};
use event_stream_infrastructure::{KafkaSettings, KafkaTail, KafkaTopic};
use tokio::time::Instant;

use crate::support::{
    CONNECT_TIMEOUT, DEAD_BOOTSTRAP_SERVERS, TestTopic, bounded, numbered, settings, unique_name,
    values,
};

/// Fetches from the tail until the batches hold `count` records, at most 20 times.
async fn fetch_records(tail: &mut KafkaTail, count: usize) -> Batch {
    let mut collected = Batch {
        records: Vec::new(),
        next: Cursor::START,
    };
    for _ in 0..20 {
        let batch = bounded(tail.fetch()).await.unwrap();
        collected.records.extend(batch.records);
        collected.next = batch.next;
        if collected.records.len() >= count {
            break;
        }
    }
    collected
}

fn offsets(batch: &Batch) -> Vec<u64> {
    batch
        .records
        .iter()
        .map(|record| record.offset.value())
        .collect()
}

#[tokio::test]
async fn the_partition_count_is_read_for_one_and_three_partitions_and_absent_for_a_missing_topic() {
    let one = TestTopic::create("count-one", 1).await;
    let three = TestTopic::create("count-three", 3).await;
    let missing = unique_name("count-missing");

    let count_one = bounded(one.readers().probe().partition_count()).await;
    let count_three = bounded(three.readers().probe().partition_count()).await;
    let missing_probe = KafkaTopic::new(settings(&missing)).probe();
    let count_missing = bounded(missing_probe.partition_count()).await;
    let still_missing = bounded(missing_probe.partition_count()).await;

    assert_eq!(count_one.unwrap(), Some(1));
    assert_eq!(count_three.unwrap(), Some(3));
    assert_eq!(count_missing.unwrap(), None);
    assert_eq!(still_missing.unwrap(), None);
    drop(missing_probe);
    one.delete().await;
    three.delete().await;
    drop((one, three));
}

#[tokio::test]
async fn the_watermarks_of_an_empty_and_of_a_filled_topic_are_those_of_the_broker() {
    let topic = TestTopic::create("watermarks", 1).await;
    let probe = topic.readers().probe();

    let empty = bounded(probe.watermarks()).await.unwrap();
    topic.produce(&numbered("record", 3)).await;
    let filled = bounded(probe.watermarks()).await.unwrap();

    assert_eq!(
        empty,
        Watermarks {
            earliest: Cursor::START,
            end: Cursor::START,
        }
    );
    assert_eq!(
        filled,
        Watermarks {
            earliest: Cursor::START,
            end: Cursor::new(3),
        }
    );
    drop(probe);
    topic.delete().await;
    drop(topic);
}

#[tokio::test]
async fn a_fetch_after_a_seek_returns_the_records_from_the_position_in_order_without_key_or_headers()
 {
    let topic = TestTopic::create("fetch", 1).await;
    topic.produce(&numbered("record", 5)).await;
    let mut tail = topic.readers().tail();

    bounded(tail.seek(Cursor::new(2))).await.unwrap();
    let batch = fetch_records(&mut tail, 3).await;

    assert_eq!(offsets(&batch), vec![2, 3, 4]);
    assert_eq!(values(&batch.records), numbered("record", 5)[2..].to_vec());
    assert_eq!(batch.next, Cursor::new(5));
    drop(tail);
    topic.delete().await;
    drop(topic);
}

#[tokio::test]
async fn a_page_returns_the_records_from_the_position_and_the_position_after_them() {
    let topic = TestTopic::create("page", 1).await;
    topic.produce(&numbered("record", 4)).await;
    let replay = topic.readers().replay();

    let page = bounded(replay.page(Cursor::new(1))).await.unwrap();
    let at_end = bounded(replay.page(Cursor::new(4))).await.unwrap();

    assert_eq!(offsets(&page), vec![1, 2, 3]);
    assert_eq!(values(&page.records), numbered("record", 4)[1..].to_vec());
    assert_eq!(page.next, Cursor::new(4));
    assert_eq!(
        at_end,
        Batch {
            records: Vec::new(),
            next: Cursor::new(4),
        }
    );
    drop(replay);
    topic.delete().await;
    drop(topic);
}

#[tokio::test]
async fn records_of_an_aborted_transaction_are_never_returned_and_the_position_passes_the_markers()
{
    let topic = TestTopic::create("read-committed", 1).await;
    topic
        .produce_in_transaction("aborted", &["lost-1", "lost-2"], false)
        .await;
    topic
        .produce_in_transaction("committed", &["kept"], true)
        .await;
    let end = bounded(topic.readers().probe().watermarks())
        .await
        .unwrap()
        .end;
    let mut tail = topic.readers().tail();
    let replay = topic.readers().replay();

    let mut fetched = Vec::new();
    let mut next = Cursor::START;
    for _ in 0..20 {
        let batch = bounded(tail.fetch()).await.unwrap();
        fetched.extend(batch.records);
        next = batch.next;
        if next == end {
            break;
        }
    }
    let page = bounded(replay.page(Cursor::START)).await.unwrap();
    drop((tail, replay));

    assert_eq!(values(&fetched), vec!["kept"]);
    assert_eq!(next, end);
    assert!(end > Cursor::new(4), "{end}");
    assert_eq!(values(&page.records), vec!["kept"]);
    assert!(page.next > Cursor::new(3), "{}", page.next);
    topic.delete().await;
    drop(topic);
}

#[tokio::test]
async fn a_position_below_the_log_start_is_below_earliest_with_the_new_log_start() {
    let topic = TestTopic::create("below-earliest", 1).await;
    topic.produce(&numbered("record", 5)).await;
    topic.delete_records_before(3).await;
    let mut tail = topic.readers().tail();
    let replay = topic.readers().replay();

    bounded(tail.seek(Cursor::new(1))).await.unwrap();
    let fetched = bounded(tail.fetch()).await;
    let paged = bounded(replay.page(Cursor::START)).await;
    bounded(tail.seek(Cursor::new(3))).await.unwrap();
    let from_start = fetch_records(&mut tail, 2).await;

    for result in [fetched, paged] {
        assert!(
            matches!(result, Err(TopicError::BelowEarliest { earliest }) if earliest == Cursor::new(3)),
            "{result:?}"
        );
    }
    assert_eq!(offsets(&from_start), vec![3, 4]);
    drop((tail, replay));
    topic.delete().await;
    drop(topic);
}

#[tokio::test]
async fn a_pooled_reader_that_served_a_higher_page_serves_a_lower_one_from_its_start() {
    let topic = TestTopic::create("pooled-reader", 1).await;
    topic.produce(&numbered("record", 1000)).await;
    let replay = topic.readers().replay();

    let high = bounded(replay.page(Cursor::new(900))).await.unwrap();
    let low = bounded(replay.page(Cursor::new(100))).await.unwrap();

    assert_eq!(
        high.records.first().map(|record| record.offset),
        Some(Offset::new(900))
    );
    assert_eq!(
        low.records.first().map(|record| record.offset),
        Some(Offset::new(100))
    );
    let last = low.records.last().unwrap().offset.value();
    assert_eq!(low.next, Cursor::new(last + 1));
    assert!(low.records.len() <= 500, "{}", low.records.len());
    assert!(
        low.records
            .windows(2)
            .all(|pair| pair[0].offset < pair[1].offset)
    );
    drop(replay);
    topic.delete().await;
    drop(topic);
}

#[tokio::test]
async fn a_page_of_large_records_stops_at_the_soft_byte_bound() {
    let topic = TestTopic::create("large-page", 1).await;
    let large = vec!["x".repeat(600 * 1024); 3];
    topic.produce(&large).await;
    let replay = topic.readers().replay();

    let page = bounded(replay.page(Cursor::START)).await.unwrap();

    let bytes: usize = page
        .records
        .iter()
        .map(|record| record.value.as_ref().map_or(0, Vec::len))
        .sum();
    assert_ne!(page.records.len(), 0);
    assert!(page.records.len() < 3, "{}", page.records.len());
    assert!(bytes <= 2 * 600 * 1024, "{bytes}");
    assert_eq!(
        page.next,
        Cursor::new(u64::try_from(page.records.len()).unwrap())
    );
    drop(replay);
    topic.delete().await;
    drop(topic);
}

#[tokio::test]
async fn an_unreachable_broker_is_unavailable_within_the_connect_time_out() {
    let settings = KafkaSettings {
        bootstrap_servers: DEAD_BOOTSTRAP_SERVERS.to_owned(),
        ..settings("unreachable")
    };
    let probe = KafkaTopic::new(settings).probe();
    let started = Instant::now();

    let result = bounded(probe.watermarks()).await;
    let elapsed = started.elapsed();
    drop(probe);

    assert!(
        matches!(result, Err(TopicError::Unavailable(_))),
        "{result:?}"
    );
    assert!(
        elapsed <= CONNECT_TIMEOUT + Duration::from_secs(1),
        "{elapsed:?}"
    );
}
