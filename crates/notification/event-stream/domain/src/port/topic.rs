//! The topic ports: the one-partition event topic, read without a consumer group.
//!
//! Rules every adapter of these ports keeps:
//!
//! 1. [`Batch::records`] holds only offsets at or above the requested position, ascending, and
//!    [`Batch::next`] is at least the requested position and at least the last offset plus 1.
//! 2. A call never waits without bound: it returns or fails with [`TopicError::Unavailable`] within
//!    the adapter's time limit.
//! 3. A position below the log start is reported as [`TopicError::BelowEarliest`], never skipped.
//! 4. The three traits are dyn-compatible and used as `Arc<dyn TopicProbe>`,
//!    `Box<dyn TopicTail>` and `Arc<dyn TopicReplay>`.
//! 5. A record batch the client cannot decode is [`TopicError::Unavailable`]: nothing is skipped.

use std::fmt::Debug;

use async_trait::async_trait;
use shared_domain::BoxError;

use crate::value_object::cursor::Cursor;
use crate::value_object::offset::Offset;
use crate::value_object::watermarks::Watermarks;

/// A failed read of the topic.
#[derive(Debug, thiserror::Error)]
pub enum TopicError {
    /// The broker could not be read: connect, request, time-out, or any other client error.
    #[error("the topic is unavailable")]
    Unavailable(#[source] BoxError),
    /// The requested position is below the log start; `earliest` is the log start read just now.
    #[error("the position is below the earliest retained offset")]
    BelowEarliest {
        /// The log start offset.
        earliest: Cursor,
    },
}

/// One record of the partition; key and headers are not carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRecord {
    /// The offset of the record.
    pub offset: Offset,
    /// The value; `None` for a record without a value.
    pub value: Option<Vec<u8>>,
}

/// Records at or above the requested position, in offset order, and the reader position after
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    /// The records, ascending by offset.
    pub records: Vec<TopicRecord>,
    /// The position to read next.
    pub next: Cursor,
}

/// The metadata reads of the topic.
#[async_trait]
pub trait TopicProbe: Debug + Send + Sync {
    /// The partition count of the topic; `None` when the metadata answer holds no such topic.
    ///
    /// # Errors
    ///
    /// Returns [`TopicError::Unavailable`] when the broker cannot be read.
    async fn partition_count(&self) -> Result<Option<u32>, TopicError>;

    /// The log start offset and the last stable offset of partition 0.
    ///
    /// # Errors
    ///
    /// Returns [`TopicError::Unavailable`] when the broker cannot be read.
    async fn watermarks(&self) -> Result<Watermarks, TopicError>;
}

/// The reader of the tail of the topic, owned by one loop.
#[async_trait]
pub trait TopicTail: Debug + Send {
    /// Sets the reader position; always calls the client's seek, which also clears its fetch
    /// backoff.
    ///
    /// # Errors
    ///
    /// Returns [`TopicError::Unavailable`] when the client refuses the seek.
    async fn seek(&mut self, position: Cursor) -> Result<(), TopicError>;

    /// One long poll from the reader position; returns early with records, or empty after the
    /// wait.
    ///
    /// # Errors
    ///
    /// Returns [`TopicError::BelowEarliest`] when the position is below the log start and
    /// [`TopicError::Unavailable`] when the broker cannot be read.
    async fn fetch(&mut self) -> Result<Batch, TopicError>;
}

/// The reader of pages of older records, shared by the replays.
#[async_trait]
pub trait TopicReplay: Debug + Send + Sync {
    /// One page of records at positions at or above `from`: a seek, then one short poll.
    ///
    /// # Errors
    ///
    /// Returns [`TopicError::BelowEarliest`] when `from` is below the log start and
    /// [`TopicError::Unavailable`] when the broker cannot be read.
    async fn page(&self, from: Cursor) -> Result<Batch, TopicError>;
}

#[cfg(test)]
mod tests;
