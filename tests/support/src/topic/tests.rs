use std::error::Error as _;
use std::sync::Arc;
use std::task::{Context, Waker};

use event_stream_domain::{
    Batch, Cursor, Offset, TopicError, TopicProbe, TopicRecord, TopicReplay, TopicTail, Watermarks,
};

use super::{ProbeCall, ScriptedTopicProbe, ScriptedTopicReplay, ScriptedTopicTail, TailCall};
use crate::block_on;

/// Polls `future` three times and checks that it never completes.
fn assert_pends<F: Future>(future: F) {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..3 {
        assert!(future.as_mut().poll(&mut context).is_pending());
    }
}

fn batch(offsets: &[u64], next: u64) -> Batch {
    Batch {
        records: offsets
            .iter()
            .map(|&offset| TopicRecord {
                offset: Offset::new(offset),
                value: Some(offset.to_string().into_bytes()),
            })
            .collect(),
        next: Cursor::new(next),
    }
}

fn unavailable(reason: &str) -> TopicError {
    TopicError::Unavailable(reason.to_owned().into())
}

fn source_of(error: &TopicError) -> Option<String> {
    error.source().map(ToString::to_string)
}

const fn watermarks(earliest: u64, end: u64) -> Watermarks {
    Watermarks {
        earliest: Cursor::new(earliest),
        end: Cursor::new(end),
    }
}

#[test]
fn the_probe_answers_each_script_in_order() {
    let probe = ScriptedTopicProbe::new();
    probe.push_partition_count(Ok(Some(1)));
    probe.push_partition_count(Ok(None));
    probe.push_partition_count(Err(unavailable("metadata")));
    probe.push_watermarks(Ok(watermarks(0, 4)));
    probe.push_watermarks(Err(unavailable("watermarks")));

    assert_eq!(block_on(probe.partition_count()).unwrap(), Some(1));
    assert_eq!(block_on(probe.watermarks()).unwrap(), watermarks(0, 4));
    assert_eq!(block_on(probe.partition_count()).unwrap(), None);
    let failed = block_on(probe.partition_count()).unwrap_err();
    let failed_watermarks = block_on(probe.watermarks()).unwrap_err();

    assert_eq!(source_of(&failed).as_deref(), Some("metadata"));
    assert_eq!(source_of(&failed_watermarks).as_deref(), Some("watermarks"));
    assert_eq!(
        probe.calls(),
        vec![
            ProbeCall::PartitionCount,
            ProbeCall::Watermarks,
            ProbeCall::PartitionCount,
            ProbeCall::PartitionCount,
            ProbeCall::Watermarks,
        ]
    );
}

#[test]
fn the_probe_records_the_call_and_pends_once_its_script_is_empty() {
    let probe = ScriptedTopicProbe::new();

    assert_pends(probe.partition_count());
    assert_eq!(probe.calls(), vec![ProbeCall::PartitionCount]);
    assert_pends(probe.watermarks());
    assert_eq!(
        probe.calls(),
        vec![ProbeCall::PartitionCount, ProbeCall::Watermarks]
    );
}

#[test]
fn the_probe_scripts_are_separate() {
    let probe = ScriptedTopicProbe::new();
    probe.push_watermarks(Ok(watermarks(2, 3)));

    assert_pends(probe.partition_count());
    assert_eq!(block_on(probe.watermarks()).unwrap(), watermarks(2, 3));
}

#[test]
fn a_probe_clone_shares_the_script_and_the_calls() {
    let probe = ScriptedTopicProbe::new();
    let handle = probe.clone();
    let shared: Arc<dyn TopicProbe> = Arc::new(probe);
    handle.push_partition_count(Ok(Some(3)));

    assert_eq!(block_on(shared.partition_count()).unwrap(), Some(3));
    assert_eq!(handle.calls(), vec![ProbeCall::PartitionCount]);
}

#[test]
fn the_tail_answers_each_script_in_order_and_records_the_calls_in_order() {
    let tail = ScriptedTopicTail::new();
    let mut owned: Box<dyn TopicTail> = Box::new(tail.clone());
    tail.push_seek(Ok(()));
    tail.push_seek(Err(unavailable("seek")));
    tail.push_fetch(Ok(batch(&[4, 5], 6)));
    tail.push_fetch(Err(TopicError::BelowEarliest {
        earliest: Cursor::new(9),
    }));

    block_on(owned.seek(Cursor::new(4))).unwrap();
    assert_eq!(block_on(owned.fetch()).unwrap(), batch(&[4, 5], 6));
    let refused = block_on(owned.seek(Cursor::new(2))).unwrap_err();
    let below = block_on(owned.fetch()).unwrap_err();

    assert_eq!(source_of(&refused).as_deref(), Some("seek"));
    assert!(matches!(below, TopicError::BelowEarliest { earliest } if earliest == Cursor::new(9)));
    assert_eq!(
        tail.calls(),
        vec![
            TailCall::Seek(Cursor::new(4)),
            TailCall::Fetch,
            TailCall::Seek(Cursor::new(2)),
            TailCall::Fetch,
        ]
    );
}

#[test]
fn the_tail_records_the_call_and_pends_once_its_script_is_empty() {
    let tail = ScriptedTopicTail::new();
    let mut owned = tail.clone();

    assert_pends(owned.seek(Cursor::new(7)));
    assert_eq!(tail.calls(), vec![TailCall::Seek(Cursor::new(7))]);
    assert_pends(owned.fetch());
    assert_eq!(
        tail.calls(),
        vec![TailCall::Seek(Cursor::new(7)), TailCall::Fetch]
    );
}

#[test]
fn the_tail_scripts_are_separate() {
    let tail = ScriptedTopicTail::new();
    let mut owned = tail.clone();
    tail.push_fetch(Ok(batch(&[], 0)));

    assert_pends(owned.seek(Cursor::START));
    assert_eq!(block_on(owned.fetch()).unwrap(), batch(&[], 0));
}

#[test]
fn the_replay_answers_in_order_and_records_the_page_positions() {
    let replay = ScriptedTopicReplay::new();
    let shared: Arc<dyn TopicReplay> = Arc::new(replay.clone());
    replay.push_page(Ok(batch(&[1, 2], 3)));
    replay.push_page(Err(unavailable("page")));

    assert_eq!(
        block_on(shared.page(Cursor::new(1))).unwrap(),
        batch(&[1, 2], 3)
    );
    let failed = block_on(shared.page(Cursor::new(3))).unwrap_err();

    assert_eq!(source_of(&failed).as_deref(), Some("page"));
    assert_eq!(replay.calls(), vec![Cursor::new(1), Cursor::new(3)]);
}

#[test]
fn the_replay_records_the_position_and_pends_once_its_script_is_empty() {
    let replay = ScriptedTopicReplay::new();

    assert_pends(replay.page(Cursor::new(11)));
    assert_eq!(replay.calls(), vec![Cursor::new(11)]);
}
