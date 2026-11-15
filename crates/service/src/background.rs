//! The background work of a node: the tail loop, its supervision and the first steps of the
//! shutdown (FR-26, FR-27).
//!
//! The loop runs [`TailEvents::round`] until it is stopped, pausing `KAFKA_RETRY_DELAY` after a
//! failed round and not at all after a successful one. The stop flag is read in one place,
//! `until_stopped`; a round never hangs because every adapter call inside it has its own time
//! limit, and a stop cuts the round in flight at once.

use std::sync::Arc;
use std::time::Duration;

use event_stream_application::Hub;
use event_stream_application::RoundFailed;
use event_stream_application::tail::TailEvents;
use tokio::sync::watch;
use tokio::task::{JoinError, JoinSet};

/// How long the shutdown of a node waits for the streams to end after the hub was closed (FR-27).
pub const STREAM_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// The name of the tail task in the log and in [`Cause::TaskFailed`].
const TAIL: &str = "tail";

/// What a node runs besides its HTTP server: the tail and the hub it feeds.
#[derive(Debug)]
pub struct Background {
    /// The use case that reads the topic into the window and the hub.
    pub tail: TailEvents,
    /// The hub of this node, closed by the shutdown.
    pub hub: Arc<Hub>,
    /// The pause after a failed round (`KAFKA_RETRY_DELAY`).
    pub retry_delay: Duration,
}

impl Background {
    /// Spawns the tail loop under supervision.
    #[must_use]
    pub fn spawn(self) -> Running {
        let (stop, stopping) = watch::channel(false);
        let mut tasks = JoinSet::new();
        start(
            &mut tasks,
            TAIL,
            tail(self.tail, self.retry_delay, stopping),
        );
        Running {
            stop,
            tasks,
            hub: self.hub,
        }
    }
}

/// Why a node stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// A signal arrived; its name.
    Signal(&'static str),
    /// A background task died; its name.
    TaskFailed(&'static str),
}

impl Cause {
    /// The outcome of the serve command for this cause.
    ///
    /// # Errors
    ///
    /// Returns [`BackgroundTaskFailed`] for [`Cause::TaskFailed`].
    pub const fn result(self) -> Result<(), BackgroundTaskFailed> {
        match self {
            Self::Signal(_) => Ok(()),
            Self::TaskFailed(task) => Err(BackgroundTaskFailed { task }),
        }
    }
}

/// A background task died; the node stops with exit status 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("background task {task} failed")]
pub struct BackgroundTaskFailed {
    /// The name of the task.
    pub task: &'static str,
}

/// The stop flag as the loop sees it.
type Stop = watch::Receiver<bool>;

/// What a supervised task reports when it ends: its name and how its inner task ended.
type Ended = (&'static str, Result<(), JoinError>);

/// The running background work. Dropping it ends the loop, because the stop flag's sender is gone.
#[derive(Debug)]
pub struct Running {
    stop: watch::Sender<bool>,
    tasks: JoinSet<Ended>,
    hub: Arc<Hub>,
}

impl Running {
    /// The signal or a dead loop, whichever is first (the signal when both are ready); writes the
    /// entry of the cause.
    pub async fn stopped<S: Future<Output = &'static str> + Send>(&mut self, signal: S) -> Cause {
        let cause = tokio::select! {
            biased;
            name = signal => Cause::Signal(name),
            task = self.failed() => Cause::TaskFailed(task),
        };
        log_cause(cause);
        cause
    }

    /// The shutdown up to the HTTP drain: the loop (a round in flight is cut), the hub (every
    /// stream ends after its queued frames and new subscriptions are refused), then the streams,
    /// for at most [`STREAM_DRAIN_TIMEOUT`].
    pub async fn shutdown(mut self) {
        self.stop.send_replace(true);
        while self.tasks.join_next().await.is_some() {}
        self.hub.close();
        drop(tokio::time::timeout(STREAM_DRAIN_TIMEOUT, self.hub.drained()).await);
    }

    /// The name of the first task that panicked; pends for ever when every task ended without a
    /// panic.
    async fn failed(&mut self) -> &'static str {
        while let Some(joined) = self.tasks.join_next().await {
            if let Ok((name, Err(_panic))) = joined {
                return name;
            }
        }
        std::future::pending().await
    }
}

/// `work`, unless the stop flag is set first (biased: the flag is checked before every poll of
/// `work`); `None` when stopped, also when the flag's sender is gone.
async fn until_stopped<F: Future + Send>(stop: &mut Stop, work: F) -> Option<F::Output> {
    tokio::select! {
        biased;
        _stopped = stop.wait_for(|stopped| *stopped) => None,
        output = work => Some(output),
    }
}

/// Spawns `work` inside an outer task that reports `name` with the way `work` ended, so that a
/// panic of `work` is seen with its name.
fn start<F: Future<Output = ()> + Send + 'static>(
    tasks: &mut JoinSet<Ended>,
    name: &'static str,
    work: F,
) {
    let inner = tokio::spawn(work);
    tasks.spawn(async move { (name, inner.await) });
}

/// `None` after a successful round, `Some(retry_delay)` after a failed one.
const fn after_round(outcome: Result<(), RoundFailed>, retry_delay: Duration) -> Option<Duration> {
    match outcome {
        Ok(()) => None,
        Err(RoundFailed) => Some(retry_delay),
    }
}

/// The tail loop: a round, then the pause it calls for, until stopped.
async fn tail(mut tail: TailEvents, retry_delay: Duration, mut stop: Stop) {
    loop {
        let Some(outcome) = until_stopped(&mut stop, tail.round()).await else {
            return;
        };
        if let Some(wait) = after_round(outcome, retry_delay)
            && until_stopped(&mut stop, tokio::time::sleep(wait))
                .await
                .is_none()
        {
            return;
        }
    }
}

fn log_cause(cause: Cause) {
    match cause {
        Cause::Signal(signal) => log_signal(signal),
        Cause::TaskFailed(task) => log_task_failed(task),
    }
}

fn log_signal(signal: &str) {
    tracing::info!(signal, "shutdown signal received");
}

fn log_task_failed(task: &str) {
    tracing::error!(task, "background task failed");
}

#[cfg(test)]
mod tests;
