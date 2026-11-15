//! The readiness probe of the event stream (FR-20).
//!
//! It lives in the composition root because the contexts `event-stream` and `health-check` may not
//! depend on each other. It reads the state the tail of this node publishes and the clock, never
//! the broker.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use event_stream_application::{TailState, TopicFault};
use health_check_domain::{Dependency, HealthProbe};
use shared_domain::{BoxError, Clock};
use tokio::sync::watch;

/// Why the event stream of this node is not ready; the text is the logged cause of the failing
/// probe, the health answer keeps the fixed message of [`Dependency::EventStream`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
enum EventStreamNotReady {
    /// The topic cannot be read during start-up; the text of the fault.
    #[error(transparent)]
    Fault(TopicFault),
    /// The tail has not reached its start goal yet.
    #[error("the node is starting")]
    Starting,
    /// The last successful watermark read is missing or older than the staleness bound.
    #[error("the topic is unreachable")]
    Unreachable,
    /// The position was last at the end longer ago than the staleness bound, or never.
    #[error("the event tail is behind")]
    Behind,
}

/// Passes while the node is started and its tail observed the topic and was at its end at most
/// `stale_after` ago, by the clock port.
#[derive(Debug)]
pub struct EventStreamProbe {
    tail: watch::Receiver<TailState>,
    clock: Arc<dyn Clock>,
    stale_after: Duration,
}

impl EventStreamProbe {
    /// A probe of the state `tail` publishes, aged by `clock` against `stale_after`
    /// (`EVENT_TAIL_STALE_AFTER`).
    #[must_use]
    pub fn new(
        tail: watch::Receiver<TailState>,
        clock: Arc<dyn Clock>,
        stale_after: Duration,
    ) -> Self {
        Self {
            tail,
            clock,
            stale_after,
        }
    }

    /// The readiness rule: the first reason that holds, in the order fault, starting, unreachable,
    /// behind; [`Self::probe`] is its only caller outside the tests of this module.
    fn readiness(&self) -> Result<(), EventStreamNotReady> {
        let state = self.tail.borrow().clone();
        let now = self.clock.now();
        if let Some(fault) = state.fault {
            return Err(EventStreamNotReady::Fault(fault));
        }
        if !state.started {
            return Err(EventStreamNotReady::Starting);
        }
        if !self.fresh(state.observed_at, now) {
            return Err(EventStreamNotReady::Unreachable);
        }
        if !self.fresh(state.caught_up_at, now) {
            return Err(EventStreamNotReady::Behind);
        }
        Ok(())
    }

    /// Whether `at` is known and at most `stale_after` old at `now`; an instant later than `now`
    /// (a wall clock stepped back) counts as age 0.
    fn fresh(&self, at: Option<SystemTime>, now: SystemTime) -> bool {
        at.is_some_and(|at| now.duration_since(at).unwrap_or(Duration::ZERO) <= self.stale_after)
    }

    /// [`Self::readiness`] as the answer of [`HealthProbe::check`], which delegates to it.
    async fn probe(&self) -> Result<(), BoxError> {
        // The rule reads a watch channel and the clock port: its answer is ready at once.
        std::future::ready(self.readiness().map_err(BoxError::from)).await
    }
}

#[async_trait::async_trait]
impl HealthProbe for EventStreamProbe {
    fn dependency(&self) -> Dependency {
        Dependency::EventStream
    }

    async fn check(&self) -> Result<(), BoxError> {
        self.probe().await
    }
}

#[cfg(test)]
mod tests;
