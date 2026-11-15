//! An in-memory partition that serves the three topic ports.

use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll};

use async_trait::async_trait;
use event_stream_domain::{
    Batch, Cursor, Offset, TopicError, TopicProbe, TopicRecord, TopicReplay, TopicTail, Watermarks,
};

#[derive(Debug)]
struct Partition {
    records: Vec<TopicRecord>,
    log_start: Cursor,
    end: Cursor,
}

impl Partition {
    /// The batch read at `position`, with the port rules: records at or above `position` in
    /// ascending order, `next` past them and never below `position`, `BelowEarliest` below the
    /// log start.
    fn read(&self, position: Cursor) -> Result<Batch, TopicError> {
        if position < self.log_start {
            return Err(TopicError::BelowEarliest {
                earliest: self.log_start,
            });
        }
        Ok(Batch {
            records: self
                .records
                .iter()
                .filter(|record| record.offset >= position.next_offset())
                .cloned()
                .collect(),
            next: position.max(self.end),
        })
    }

    /// Keeps the records in offset order and the end past the last of them.
    fn insert(&mut self, record: TopicRecord) {
        self.end = self.end.max(Cursor::from(record.offset.sequence()));
        let at = self
            .records
            .partition_point(|held| held.offset < record.offset);
        self.records.insert(at, record);
    }
}

/// A future that is pending once, waking its task first, then ready: it lets a loop on an empty
/// fetch yield to the other tasks.
#[derive(Debug, Default)]
struct YieldOnce {
    yielded: bool,
}

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.yielded {
            return Poll::Ready(());
        }
        self.yielded = true;
        context.waker().wake_by_ref();
        Poll::Pending
    }
}

/// An in-memory partition with records, a log start and an end that serves the three topic ports.
///
/// The end is the last stable offset. [`TopicProbe`], [`TopicTail`] and [`TopicReplay`] keep the
/// port rules. A clone is a shared handle on the same partition with a reader position of its own,
/// so a test can append a record or move the log start while the code under test reads.
#[derive(Debug, Clone)]
pub struct StaticTopic {
    partition: Arc<Mutex<Partition>>,
    position: Cursor,
}

impl StaticTopic {
    /// A partition holding `records` from `log_start` to `end`. Records below the log start are
    /// dropped, and the end moves past the last record when it is lower.
    #[must_use]
    pub fn new(records: Vec<TopicRecord>, log_start: Cursor, end: Cursor) -> Self {
        let mut partition = Partition {
            records: Vec::with_capacity(records.len()),
            log_start,
            end: end.max(log_start),
        };
        for record in records
            .into_iter()
            .filter(|record| record.offset >= log_start.next_offset())
        {
            partition.insert(record);
        }
        Self {
            partition: Arc::new(Mutex::new(partition)),
            position: Cursor::START,
        }
    }

    /// An empty partition: log start and end at 0.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new(), Cursor::START, Cursor::START)
    }

    fn partition(&self) -> MutexGuard<'_, Partition> {
        self.partition
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Appends a record at the end and returns its offset.
    #[must_use]
    pub fn append(&self, value: Option<Vec<u8>>) -> Offset {
        let mut partition = self.partition();
        let offset = partition.end.next_offset();
        partition.insert(TopicRecord { offset, value });
        offset
    }

    /// Moves the end to `end` without a record, as transaction markers do; never lowers it.
    pub fn advance_end(&self, end: Cursor) {
        let mut partition = self.partition();
        partition.end = partition.end.max(end);
    }

    /// Moves the log start to `log_start` and drops the records below it, as retention does; the
    /// end follows when it is lower.
    pub fn move_log_start(&self, log_start: Cursor) {
        let mut partition = self.partition();
        partition.log_start = log_start;
        partition.end = partition.end.max(log_start);
        partition
            .records
            .retain(|record| record.offset >= log_start.next_offset());
    }

    /// The reader position of this handle.
    #[must_use]
    pub const fn position(&self) -> Cursor {
        self.position
    }

    fn read(&self, position: Cursor) -> Result<Batch, TopicError> {
        self.partition().read(position)
    }

    fn current_watermarks(&self) -> Watermarks {
        let partition = self.partition();
        Watermarks {
            earliest: partition.log_start,
            end: partition.end,
        }
    }

    const fn move_to(&mut self, position: Cursor) {
        self.position = position;
    }

    async fn fetch_from_position(&mut self) -> Result<Batch, TopicError> {
        let batch = self.read(self.position)?;
        if batch.records.is_empty() {
            YieldOnce::default().await;
        }
        self.position = batch.next;
        Ok(batch)
    }
}

#[async_trait]
impl TopicProbe for StaticTopic {
    async fn partition_count(&self) -> Result<Option<u32>, TopicError> {
        Ok(Some(1))
    }

    async fn watermarks(&self) -> Result<Watermarks, TopicError> {
        Ok(self.current_watermarks())
    }
}

#[async_trait]
impl TopicTail for StaticTopic {
    async fn seek(&mut self, position: Cursor) -> Result<(), TopicError> {
        self.move_to(position);
        Ok(())
    }

    async fn fetch(&mut self) -> Result<Batch, TopicError> {
        self.fetch_from_position().await
    }
}

#[async_trait]
impl TopicReplay for StaticTopic {
    async fn page(&self, from: Cursor) -> Result<Batch, TopicError> {
        self.read(from)
    }
}

#[cfg(test)]
mod tests;
