//! The tail of the event topic and its use case [`TailEvents`].
//!
//! [`TailEvents`] reads the topic into the memory window and the hub; this module also holds the
//! trait through which it reaches the hub, its settings and ports, the faults of the topic and the
//! state it publishes after every round.

use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use event_stream_domain::{
    Batch, Cursor, Offset, StoredEvent, TopicError, TopicProbe, TopicRecord, TopicTail, Watermarks,
};
use shared_domain::{Clock, error_chain};
use tokio::sync::watch;
use tokio::time::Instant;

use crate::envelope::{RejectReason, parse_event};
use crate::window::Window;

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

/// Why a round failed: a read of the topic, or a topic that cannot be used (start-up only).
#[derive(Debug, thiserror::Error)]
enum RoundError {
    /// A read of the topic failed.
    #[error(transparent)]
    Read(#[from] TopicError),
    /// The broker answered, but the topic does not exist or has another partition count.
    #[error(transparent)]
    Fault(TopicFault),
}

impl RoundError {
    /// The fault of the topic that a failed start-up round shows.
    const fn fault(&self) -> TopicFault {
        match self {
            Self::Read(_) => TopicFault::Unreachable,
            Self::Fault(fault) => *fault,
        }
    }
}

/// The first error of a tail round; the later steps of the round still run.
#[derive(Debug, Default)]
struct FirstError(Option<TopicError>);

impl FirstError {
    /// Keeps the error of `result` unless an earlier step already failed.
    fn keep(&mut self, result: Result<(), TopicError>) {
        if let Err(error) = result
            && self.0.is_none()
        {
            self.0 = Some(error);
        }
    }

    /// The outcome of the round: the position it reached, or its first error.
    fn into_result(self, position: Cursor) -> Result<Cursor, RoundError> {
        self.0
            .map_or(Ok(position), |error| Err(RoundError::Read(error)))
    }
}

/// What the fetch of a tail round gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fetched {
    /// A batch with at least one record.
    Records,
    /// A batch without records.
    Empty,
    /// The position was below the log start and jumped to it.
    Jumped,
    /// The topic was unavailable.
    Failed,
}

/// What the watermark observation of a tail round gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Observed {
    /// The round did not need the watermarks.
    Skipped,
    /// The read failed.
    Failed,
    /// The read passed; `after_failure` when the read before it had failed.
    Read {
        /// The end of the topic.
        end: Cursor,
        /// The previous watermark read had failed.
        after_failure: bool,
    },
}

/// Whether the rounds of the tail have succeeded, for the transition log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Health {
    /// No round has succeeded yet.
    Unproven,
    /// The last round succeeded.
    Succeeding,
    /// The rounds fail since one that succeeded; `event tail failed` was written.
    Failing,
}

/// Reads the topic into the memory window and the publisher, one round at a time: the start-up
/// steps until the first seek, then tail rounds ("Algorithms" 1 and 2 of the design).
///
/// The loop around [`TailEvents::round`] pauses after a failed round and stops the tail; every
/// log entry of the tail is written here (NFR-15). Times come only from the clock port. The
/// sender of the published [`TailState`] is dropped with the tail.
#[derive(Debug)]
pub struct TailEvents {
    probe: Arc<dyn TopicProbe>,
    tail: Box<dyn TopicTail>,
    clock: Arc<dyn Clock>,
    window: Arc<Window>,
    publisher: Arc<dyn EventPublisher>,
    sender: watch::Sender<TailState>,
    state: TailState,
    settings: TailSettings,
    goal: Cursor,
    last_attempt: Instant,
    health: Health,
    last_fault: Option<TopicFault>,
}

impl TailEvents {
    /// The watermark period of the tail rounds (FR-21).
    pub const OBSERVE_EVERY: Duration = Duration::from_secs(1);

    /// The tail and the receiver of the state it publishes after every round.
    #[must_use]
    pub fn new(
        ports: TailPorts,
        window: Arc<Window>,
        publisher: Arc<dyn EventPublisher>,
        settings: TailSettings,
    ) -> (Self, watch::Receiver<TailState>) {
        let (sender, receiver) = watch::channel(TailState::default());
        let events = Self {
            probe: ports.probe,
            tail: ports.tail,
            clock: ports.clock,
            window,
            publisher,
            sender,
            state: TailState::default(),
            settings,
            goal: Cursor::START,
            last_attempt: Instant::now(),
            health: Health::Unproven,
            last_fault: None,
        };
        (events, receiver)
    }

    /// One round: the start-up steps until the first seek, then a tail round. The state is
    /// published at the end of every round.
    ///
    /// # Errors
    ///
    /// Returns [`RoundFailed`] when a step of the round failed; the loop pauses before the next
    /// round.
    pub async fn round(&mut self) -> Result<(), RoundFailed> {
        let result = match self.state.position {
            None => self.start_up().await,
            Some(position) => self.tail_round(position).await,
        };
        self.note_health(&result);
        self.sender.send_replace(self.state.clone());
        if result.is_ok() {
            Ok(())
        } else {
            Err(RoundFailed)
        }
    }

    /// The start-up steps; a fault is recorded and logged when it changes.
    async fn start_up(&mut self) -> Result<Cursor, RoundError> {
        let result = self.first_seek().await;
        if let Err(error) = &result {
            self.note_fault(error);
        }
        result
    }

    /// The metadata check, the first observation, the start position and the first seek.
    async fn first_seek(&mut self) -> Result<Cursor, RoundError> {
        check_topic(self.probe.as_ref()).await?;
        let watermarks = self.observe().await?;
        let start = watermarks
            .earliest
            .max(watermarks.end.rewound_by(self.settings.start_events));
        self.window.restart(start);
        self.tail.seek(start).await?;
        self.state.position = Some(start);
        self.state.fault = None;
        self.catch_up(start);
        Ok(start)
    }

    /// Records the fault of a failed start-up round and logs it when it differs from the last.
    fn note_fault(&mut self, error: &RoundError) {
        let fault = error.fault();
        if fault == TopicFault::Unreachable {
            self.state.broker_reachable = false;
        }
        self.state.fault = Some(fault);
        if self.last_fault != Some(fault) {
            self.last_fault = Some(fault);
            log_fault(fault, error);
        }
    }

    /// The fetch, the observation when it is due and the re-seek when it is due.
    async fn tail_round(&mut self, from: Cursor) -> Result<Cursor, RoundError> {
        let mut errors = FirstError::default();
        let (position, fetched) = match self.tail.fetch().await {
            Ok(batch) => self.take_batch(&batch),
            Err(TopicError::BelowEarliest { earliest }) => (
                self.jump(from, earliest, &mut errors).await,
                Fetched::Jumped,
            ),
            Err(error) => {
                errors.keep(Err(error));
                (from, Fetched::Failed)
            }
        };
        self.state.position = Some(position);
        let observed = self.observe_when_due(fetched, &mut errors).await;
        if reseek_due(fetched, observed, position) {
            errors.keep(self.tail.seek(position).await);
        }
        errors.into_result(position)
    }

    /// Hands the records of the batch to the window and, once started, to the publisher; the
    /// position becomes the batch's next position.
    fn take_batch(&mut self, batch: &Batch) -> (Cursor, Fetched) {
        let fetched = if batch.records.is_empty() {
            Fetched::Empty
        } else {
            Fetched::Records
        };
        let behind = batch
            .records
            .iter()
            .map(|record| self.take_record(record))
            .fold(0, usize::saturating_add);
        self.publisher.advance(batch.next);
        if behind > 0 {
            log_fell_behind(behind);
        }
        self.catch_up(batch.next);
        (batch.next, fetched)
    }

    /// One record: a valid event is held, an invalid record is counted and logged. Returns how
    /// many subscribers fell behind with it.
    fn take_record(&mut self, record: &TopicRecord) -> usize {
        let value = record.value.as_deref();
        match parse_event(record.offset, value, self.settings.max_data_bytes) {
            Ok(event) => self.hold(event),
            Err(reason) => {
                self.reject(record.offset, reason);
                0
            }
        }
    }

    /// Pushes the event to the window first and then, once started, publishes it.
    fn hold(&self, event: StoredEvent) -> usize {
        let event = Arc::new(event);
        self.window.push(Arc::clone(&event));
        if self.state.started {
            self.publisher.publish(&event)
        } else {
            0
        }
    }

    fn reject(&mut self, offset: Offset, reason: RejectReason) {
        self.state.rejected = self.state.rejected.saturating_add(1);
        log_record_rejected(offset, reason);
    }

    /// Retention overtook the position: it jumps to the log start.
    async fn jump(&mut self, from: Cursor, earliest: Cursor, errors: &mut FirstError) -> Cursor {
        log_lost_to_retention(from, earliest);
        self.state.retention_losses = self.state.retention_losses.saturating_add(1);
        errors.keep(self.tail.seek(earliest).await);
        self.publisher.advance(earliest);
        earliest
    }

    /// Reads the watermarks when the fetch brought no record or the last attempt is
    /// [`Self::OBSERVE_EVERY`] old.
    async fn observe_when_due(&mut self, fetched: Fetched, errors: &mut FirstError) -> Observed {
        if fetched == Fetched::Records && self.last_attempt.elapsed() < Self::OBSERVE_EVERY {
            return Observed::Skipped;
        }
        let after_failure = !self.state.broker_reachable;
        match self.observe().await {
            Ok(watermarks) => Observed::Read {
                end: watermarks.end,
                after_failure,
            },
            Err(error) => {
                errors.keep(Err(error));
                Observed::Failed
            }
        }
    }

    /// One watermark read, recorded in the state.
    async fn observe(&mut self) -> Result<Watermarks, TopicError> {
        self.last_attempt = Instant::now();
        let read = self.probe.watermarks().await;
        match &read {
            Ok(watermarks) => self.note_watermarks(*watermarks),
            Err(_) => self.state.broker_reachable = false,
        }
        read
    }

    fn note_watermarks(&mut self, watermarks: Watermarks) {
        self.goal = watermarks.end;
        self.state.broker_reachable = true;
        self.state.earliest = Some(watermarks.earliest);
        self.state.end = Some(watermarks.end);
        self.state.observations = self.state.observations.saturating_add(1);
        self.state.observed_at = Some(self.clock.now());
    }

    /// At or above the goal: the node caught up now and, the first time, it is started.
    fn catch_up(&mut self, position: Cursor) {
        if position < self.goal {
            return;
        }
        self.state.caught_up_at = Some(self.clock.now());
        if !self.state.started {
            self.state.started = true;
            self.publisher.started(position);
            log_tail_ready(position);
        }
    }

    /// Logs the first failed round after a success and the first success after a logged failure.
    fn note_health(&mut self, result: &Result<Cursor, RoundError>) {
        self.health = match (self.health, result) {
            (Health::Failing, Ok(position)) => {
                log_tail_recovered(*position);
                Health::Succeeding
            }
            (Health::Unproven | Health::Succeeding, Ok(_)) => Health::Succeeding,
            (Health::Succeeding, Err(error)) => {
                log_tail_failed(error);
                Health::Failing
            }
            (health @ (Health::Unproven | Health::Failing), Err(_)) => health,
        };
    }
}

/// The topic must exist with exactly one partition; the node never creates it (FR-17).
async fn check_topic(probe: &dyn TopicProbe) -> Result<(), RoundError> {
    match probe.partition_count().await? {
        Some(1) => Ok(()),
        Some(count) => Err(RoundError::Fault(TopicFault::Partitions(count))),
        None => Err(RoundError::Fault(TopicFault::Missing)),
    }
}

/// The re-seek of a tail round clears the client's fetch backoff (FR-21): after a failed fetch,
/// after a successful watermark read that follows a failed one, and after an empty fetch below
/// the end.
fn reseek_due(fetched: Fetched, observed: Observed, position: Cursor) -> bool {
    let fetch_failed = fetched == Fetched::Failed;
    match observed {
        Observed::Read { end, after_failure } => {
            fetch_failed || after_failure || (fetched == Fetched::Empty && end > position)
        }
        Observed::Skipped | Observed::Failed => fetch_failed,
    }
}

fn log_fault(fault: TopicFault, error: &RoundError) {
    match fault {
        TopicFault::Unreachable => log_topic_unreachable(error),
        TopicFault::Missing => log_topic_invalid("topic-not-found"),
        TopicFault::Partitions(_) => log_topic_invalid("partition-count"),
    }
}

fn log_topic_unreachable(error: &RoundError) {
    tracing::warn!(error = %error_chain(error), "event topic unreachable");
}

fn log_topic_invalid(reason: &'static str) {
    tracing::error!(reason, "event topic invalid");
}

fn log_tail_ready(position: Cursor) {
    tracing::info!(position = position.value(), "event tail ready");
}

fn log_tail_failed(error: &RoundError) {
    tracing::warn!(error = %error_chain(error), "event tail failed");
}

fn log_tail_recovered(position: Cursor) {
    tracing::info!(position = position.value(), "event tail recovered");
}

fn log_lost_to_retention(from: Cursor, to: Cursor) {
    let distance = to.distance_from(from);
    let (from, to) = (from.value(), to.value());
    tracing::error!(from, to, distance, "events lost to retention");
}

fn log_record_rejected(offset: Offset, reason: RejectReason) {
    tracing::warn!(offset = offset.value(), reason = %reason, "event record rejected");
}

fn log_fell_behind(count: usize) {
    tracing::warn!(count, "subscribers fell behind");
}

#[cfg(test)]
mod tests;
