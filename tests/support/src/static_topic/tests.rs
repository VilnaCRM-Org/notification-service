use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use event_stream_domain::{
    Batch, Cursor, Offset, TopicError, TopicProbe as _, TopicRecord, TopicReplay as _,
    TopicTail as _, Watermarks,
};

use super::{StaticTopic, YieldOnce};
use crate::block_on;

/// A waker that counts its wakes.
#[derive(Debug, Default)]
struct CountingWaker(AtomicUsize);

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn record(offset: u64) -> TopicRecord {
    TopicRecord {
        offset: Offset::new(offset),
        value: Some(format!("value {offset}").into_bytes()),
    }
}

fn offsets(batch: &Batch) -> Vec<u64> {
    batch
        .records
        .iter()
        .map(|record| record.offset.value())
        .collect()
}

/// Records at 3, 4 and 6 (5 is a marker), log start 3, end 8.
fn topic() -> StaticTopic {
    StaticTopic::new(
        vec![record(6), record(3), record(4)],
        Cursor::new(3),
        Cursor::new(8),
    )
}

fn below_earliest(answer: &Result<Batch, TopicError>) -> Option<Cursor> {
    match answer {
        Err(TopicError::BelowEarliest { earliest }) => Some(*earliest),
        _ => None,
    }
}

const fn watermarks(earliest: u64, end: u64) -> Watermarks {
    Watermarks {
        earliest: Cursor::new(earliest),
        end: Cursor::new(end),
    }
}

#[test]
fn the_probe_reports_one_partition_and_the_two_ends() {
    let topic = topic();

    assert_eq!(block_on(topic.partition_count()).unwrap(), Some(1));
    assert_eq!(block_on(topic.watermarks()).unwrap(), watermarks(3, 8));
}

#[test]
fn an_empty_topic_starts_and_ends_at_zero() {
    let topic = StaticTopic::empty();

    assert_eq!(block_on(topic.watermarks()).unwrap(), watermarks(0, 0));
    assert_eq!(
        block_on(topic.page(Cursor::START)).unwrap(),
        Batch {
            records: Vec::new(),
            next: Cursor::START,
        }
    );
}

#[test]
fn a_page_holds_the_records_at_or_above_the_position_in_order() {
    let topic = topic();

    let from_start = block_on(topic.page(Cursor::new(3))).unwrap();
    let from_four = block_on(topic.page(Cursor::new(4))).unwrap();
    let from_marker = block_on(topic.page(Cursor::new(5))).unwrap();

    assert_eq!(offsets(&from_start), vec![3, 4, 6]);
    assert_eq!(from_start.records[0], record(3));
    assert_eq!(offsets(&from_four), vec![4, 6]);
    assert_eq!(offsets(&from_marker), vec![6]);
    for batch in [from_start, from_four, from_marker] {
        assert_eq!(batch.next, Cursor::new(8));
    }
}

#[test]
fn a_page_past_the_records_moves_to_the_end() {
    let topic = topic();

    let batch = block_on(topic.page(Cursor::new(7))).unwrap();

    assert_eq!(batch.records, Vec::new());
    assert_eq!(batch.next, Cursor::new(8));
}

#[test]
fn a_page_at_or_above_the_end_is_empty_and_keeps_the_position() {
    let topic = topic();

    for position in [8, 9, 100] {
        let batch = block_on(topic.page(Cursor::new(position))).unwrap();

        assert_eq!(batch.records, Vec::new());
        assert_eq!(batch.next, Cursor::new(position));
    }
}

#[test]
fn a_page_below_the_log_start_names_the_log_start() {
    let topic = topic();

    assert_eq!(
        below_earliest(&block_on(topic.page(Cursor::new(2)))),
        Some(Cursor::new(3))
    );
    assert_eq!(
        below_earliest(&block_on(topic.page(Cursor::START))),
        Some(Cursor::new(3))
    );
}

#[test]
fn new_drops_the_records_below_the_log_start_and_moves_the_end_past_the_last_record() {
    let topic = StaticTopic::new(
        vec![record(1), record(2), record(5)],
        Cursor::new(2),
        Cursor::new(3),
    );

    assert_eq!(block_on(topic.watermarks()).unwrap(), watermarks(2, 6));
    assert_eq!(
        offsets(&block_on(topic.page(Cursor::new(2))).unwrap()),
        vec![2, 5]
    );
}

#[test]
fn new_keeps_the_end_at_or_above_the_log_start() {
    let topic = StaticTopic::new(Vec::new(), Cursor::new(5), Cursor::new(2));

    assert_eq!(block_on(topic.watermarks()).unwrap(), watermarks(5, 5));
}

#[test]
fn append_writes_at_the_end_through_any_handle() {
    let topic = topic();
    let reader = topic.clone();

    assert_eq!(topic.append(Some(b"nine".to_vec())), Offset::new(8));
    assert_eq!(topic.append(None), Offset::new(9));

    let batch = block_on(reader.page(Cursor::new(8))).unwrap();
    assert_eq!(
        batch.records,
        vec![
            TopicRecord {
                offset: Offset::new(8),
                value: Some(b"nine".to_vec()),
            },
            TopicRecord {
                offset: Offset::new(9),
                value: None,
            },
        ]
    );
    assert_eq!(batch.next, Cursor::new(10));
    assert_eq!(block_on(reader.watermarks()).unwrap(), watermarks(3, 10));
}

#[test]
fn advance_end_moves_the_end_without_a_record_and_never_lowers_it() {
    let topic = topic();

    topic.advance_end(Cursor::new(11));
    topic.advance_end(Cursor::new(4));

    assert_eq!(block_on(topic.watermarks()).unwrap(), watermarks(3, 11));
    assert_eq!(topic.append(None), Offset::new(11));
}

#[test]
fn move_log_start_drops_the_older_records_through_any_handle() {
    let topic = topic();
    let reader = topic.clone();

    topic.move_log_start(Cursor::new(4));

    assert_eq!(block_on(reader.watermarks()).unwrap(), watermarks(4, 8));
    assert_eq!(
        below_earliest(&block_on(reader.page(Cursor::new(3)))),
        Some(Cursor::new(4))
    );
    assert_eq!(
        offsets(&block_on(reader.page(Cursor::new(4))).unwrap()),
        vec![4, 6]
    );
}

#[test]
fn move_log_start_past_the_end_moves_the_end() {
    let topic = topic();

    topic.move_log_start(Cursor::new(10));

    assert_eq!(block_on(topic.watermarks()).unwrap(), watermarks(10, 10));
    assert_eq!(
        block_on(topic.page(Cursor::new(10))).unwrap().records,
        Vec::new()
    );
}

#[test]
fn the_tail_reads_from_its_seek_position_and_moves_past_the_batch() {
    let mut tail = topic();

    block_on(tail.seek(Cursor::new(4))).unwrap();
    assert_eq!(tail.position(), Cursor::new(4));
    let batch = block_on(tail.fetch()).unwrap();

    assert_eq!(offsets(&batch), vec![4, 6]);
    assert_eq!(batch.next, Cursor::new(8));
    assert_eq!(tail.position(), Cursor::new(8));
}

#[test]
fn a_fetch_with_records_answers_at_once() {
    let mut tail = topic();
    block_on(tail.seek(Cursor::new(3))).unwrap();
    let waker = Arc::new(CountingWaker::default());
    let counted = Waker::from(Arc::clone(&waker));
    let mut context = Context::from_waker(&counted);

    let mut fetch = std::pin::pin!(tail.fetch());
    let Poll::Ready(Ok(batch)) = fetch.as_mut().poll(&mut context) else {
        panic!("a fetch with records must answer on the first poll");
    };

    assert_eq!(offsets(&batch), vec![3, 4, 6]);
    assert_eq!(waker.0.load(Ordering::SeqCst), 0);
}

#[test]
fn an_empty_fetch_pends_once_and_wakes_itself_before_it_answers() {
    let mut tail = topic();
    block_on(tail.seek(Cursor::new(8))).unwrap();
    let waker = Arc::new(CountingWaker::default());
    let counted = Waker::from(Arc::clone(&waker));
    let mut context = Context::from_waker(&counted);

    let answer = {
        let mut fetch = std::pin::pin!(tail.fetch());
        assert!(fetch.as_mut().poll(&mut context).is_pending());
        assert_eq!(waker.0.load(Ordering::SeqCst), 1);
        fetch.as_mut().poll(&mut context)
    };

    let Poll::Ready(Ok(batch)) = answer else {
        panic!("the empty fetch must answer on the second poll");
    };
    assert_eq!(batch.records, Vec::new());
    assert_eq!(batch.next, Cursor::new(8));
    assert_eq!(tail.position(), Cursor::new(8));
    assert_eq!(waker.0.load(Ordering::SeqCst), 1);
}

#[test]
fn a_fetch_sees_a_record_appended_through_another_handle() {
    let mut tail = topic();
    let writer = tail.clone();
    block_on(tail.seek(Cursor::new(8))).unwrap();
    assert_eq!(block_on(tail.fetch()).unwrap().records, Vec::new());

    assert_eq!(writer.append(Some(b"late".to_vec())), Offset::new(8));
    let batch = block_on(tail.fetch()).unwrap();

    assert_eq!(offsets(&batch), vec![8]);
    assert_eq!(tail.position(), Cursor::new(9));
}

#[test]
fn a_fetch_below_the_log_start_keeps_the_position() {
    let mut tail = topic();

    let answer = block_on(tail.fetch());

    assert_eq!(below_earliest(&answer), Some(Cursor::new(3)));
    assert_eq!(tail.position(), Cursor::START);
}

#[test]
fn every_handle_has_its_own_position() {
    let mut first = topic();
    let mut second = first.clone();

    block_on(first.seek(Cursor::new(6))).unwrap();
    block_on(second.seek(Cursor::new(3))).unwrap();
    assert_eq!(offsets(&block_on(first.fetch()).unwrap()), vec![6]);

    assert_eq!(second.position(), Cursor::new(3));
    assert_eq!(offsets(&block_on(second.fetch()).unwrap()), vec![3, 4, 6]);
}

#[test]
fn yield_once_is_pending_once_after_a_wake_then_ready() {
    let waker = Arc::new(CountingWaker::default());
    let counted = Waker::from(Arc::clone(&waker));
    let mut context = Context::from_waker(&counted);
    let mut future = std::pin::pin!(YieldOnce::default());

    assert!(future.as_mut().poll(&mut context).is_pending());
    assert_eq!(waker.0.load(Ordering::SeqCst), 1);
    assert!(future.as_mut().poll(&mut context).is_ready());
    assert_eq!(waker.0.load(Ordering::SeqCst), 1);
}
