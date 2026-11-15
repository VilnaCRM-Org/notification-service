//! The types of the tail of the event topic: the trait through which it reaches the hub, its
//! settings and ports, the faults of the topic and the state it publishes after every round.

use std::fmt::Debug;
use std::sync::Arc;
use std::time::SystemTime;

use event_stream_domain::{Cursor, StoredEvent, TopicProbe, TopicTail};
use shared_domain::Clock;

/// The receiver of the events the tail reads: the hub of this node.
pub trait EventPublisher: Debug + Send + Sync {
    /// The node reached its start goal: from now on the publisher accepts subscribers. Called once.
    fn started(&self, position: Cursor);

    /// Hands one event to the local subscribers. Never waits. Returns how many fell behind with it.
    fn publish(&self, event: &StoredEvent) -> usize;

    /// The tail passed offsets without a valid event (markers, invalid records): the position
    /// becomes the max of the current one and `position`.
    fn advance(&self, position: Cursor);
}

/// The settings of the tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TailSettings {
    /// The largest record value the tail accepts, in bytes.
    pub max_data_bytes: usize,
    /// How many of the most recent offsets the node reads before it is ready (FR-20).
    pub start_events: u64,
}

/// The ports the tail uses; one struct so that its constructor stays under five arguments.
#[derive(Debug)]
pub struct TailPorts {
    /// The metadata reads of the topic.
    pub probe: Arc<dyn TopicProbe>,
    /// The reader of the tail of the topic.
    pub tail: Box<dyn TopicTail>,
    /// The source of the times the state records.
    pub clock: Arc<dyn Clock>,
}

/// Why a node cannot read its topic (design brief 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TopicFault {
    /// The broker could not be read.
    #[error("the topic is unreachable")]
    Unreachable,
    /// The broker answered without the topic.
    #[error("the topic does not exist")]
    Missing,
    /// The topic has this number of partitions instead of one.
    #[error("the topic must have exactly one partition")]
    Partitions(u32),
}

/// What the tail publishes after every round (a `tokio::sync::watch` value; the tail drops the
/// sender when it stops).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TailState {
    /// Why the topic cannot be read, `None` when it can.
    pub fault: Option<TopicFault>,
    /// The position reached the start goal once (FR-20); never reset.
    pub started: bool,
    /// The reader position P; `None` before the first seek.
    pub position: Option<Cursor>,
    /// The log start L of the last successful watermark read.
    pub earliest: Option<Cursor>,
    /// The end E of the last successful watermark read.
    pub end: Option<Cursor>,
    /// The last watermark read succeeded.
    pub broker_reachable: bool,
    /// Successful watermark reads since the process started.
    pub observations: u64,
    /// The time of the last successful watermark read (from the `Clock` port).
    pub observed_at: Option<SystemTime>,
    /// The time of the last batch after which the position reached the goal (from the `Clock`
    /// port).
    pub caught_up_at: Option<SystemTime>,
    /// Invalid records skipped by the tail.
    pub rejected: u64,
    /// Jumps to the log start because retention overtook the tail.
    pub retention_losses: u64,
}

/// A round of the tail failed; the loop pauses before the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the round failed")]
pub struct RoundFailed;

#[cfg(test)]
mod tests;
