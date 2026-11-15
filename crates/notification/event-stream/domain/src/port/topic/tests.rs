use std::error::Error as _;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use async_trait::async_trait;

use super::{Batch, TopicError, TopicProbe, TopicRecord, TopicReplay, TopicTail};
use crate::value_object::cursor::Cursor;
use crate::value_object::offset::Offset;
use crate::value_object::watermarks::Watermarks;

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

const WATERMARKS: Watermarks = Watermarks {
    earliest: Cursor::new(2),
    end: Cursor::new(5),
};

/// A one-record topic, enough to prove that the traits are used as trait objects.
#[derive(Debug)]
struct OneRecord {
    position: Cursor,
}

const fn one_record() -> OneRecord {
    OneRecord {
        position: Cursor::START,
    }
}

fn batch_from(position: Cursor) -> Result<Batch, TopicError> {
    if position < WATERMARKS.earliest {
        return Err(TopicError::BelowEarliest {
            earliest: WATERMARKS.earliest,
        });
    }
    Ok(Batch {
        records: vec![TopicRecord {
            offset: Offset::new(4),
            value: Some(b"{}".to_vec()),
        }],
        next: WATERMARKS.end,
    })
}

#[async_trait]
impl TopicProbe for OneRecord {
    async fn partition_count(&self) -> Result<Option<u32>, TopicError> {
        Ok(Some(1))
    }

    async fn watermarks(&self) -> Result<Watermarks, TopicError> {
        Ok(WATERMARKS)
    }
}

#[async_trait]
impl TopicTail for OneRecord {
    async fn seek(&mut self, position: Cursor) -> Result<(), TopicError> {
        self.position = position;
        Ok(())
    }

    async fn fetch(&mut self) -> Result<Batch, TopicError> {
        batch_from(self.position)
    }
}

#[async_trait]
impl TopicReplay for OneRecord {
    async fn page(&self, from: Cursor) -> Result<Batch, TopicError> {
        batch_from(from)
    }
}

#[test]
fn the_probe_is_used_as_a_shared_trait_object() {
    let probe: Arc<dyn TopicProbe> = Arc::new(one_record());

    assert_eq!(block_on(probe.partition_count()).unwrap(), Some(1));
    assert_eq!(block_on(probe.watermarks()).unwrap(), WATERMARKS);
}

#[test]
fn the_tail_is_used_as_an_owned_trait_object() {
    let mut tail: Box<dyn TopicTail> = Box::new(one_record());

    assert!(matches!(
        block_on(tail.fetch()),
        Err(TopicError::BelowEarliest { earliest }) if earliest == Cursor::new(2)
    ));
    block_on(tail.seek(Cursor::new(3))).unwrap();
    let batch = block_on(tail.fetch()).unwrap();

    assert_eq!(batch.next, Cursor::new(5));
    assert_eq!(batch.records[0].offset, Offset::new(4));
}

#[test]
fn the_replay_is_used_as_a_shared_trait_object() {
    let replay: Arc<dyn TopicReplay> = Arc::new(one_record());

    let batch = block_on(replay.page(Cursor::new(2))).unwrap();

    assert_eq!(
        batch.records,
        vec![TopicRecord {
            offset: Offset::new(4),
            value: Some(b"{}".to_vec()),
        }]
    );
}

#[test]
fn unavailable_keeps_its_source_in_the_error_chain() {
    let error = TopicError::Unavailable("connection refused".into());

    assert_eq!(error.to_string(), "the topic is unavailable");
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("connection refused")
    );
}

#[test]
fn below_earliest_names_the_rule_and_has_no_source() {
    let error = TopicError::BelowEarliest {
        earliest: Cursor::new(9),
    };

    assert_eq!(
        error.to_string(),
        "the position is below the earliest retained offset"
    );
    assert!(error.source().is_none());
}

#[test]
fn a_topic_error_has_exactly_two_variants() {
    let describe = |error: &TopicError| match error {
        TopicError::Unavailable(_) => "unavailable",
        TopicError::BelowEarliest { .. } => "below earliest",
    };

    assert_eq!(
        describe(&TopicError::Unavailable("x".into())),
        "unavailable"
    );
    assert_eq!(
        describe(&TopicError::BelowEarliest {
            earliest: Cursor::START
        }),
        "below earliest"
    );
}

#[test]
fn a_record_may_have_no_value() {
    let record = TopicRecord {
        offset: Offset::new(0),
        value: None,
    };

    assert_eq!(record.clone(), record);
    assert!(record.value.is_none());
}
