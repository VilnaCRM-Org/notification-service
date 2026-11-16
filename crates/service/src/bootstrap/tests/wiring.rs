//! The wiring that the tests of this crate share: the adapters of a node on a `StaticTopic`, the
//! pacing adapter of its tail and the values of the records of its topic.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use event_stream_domain::{Batch, Cursor, TopicError, TopicTail};
use health_check_domain::{Dependency, HealthProbe};
use test_support::clock::FixedClock;
use test_support::id::SequenceUuidGenerator;
use test_support::probe::StubProbe;
use test_support::static_topic::StaticTopic;

use crate::bootstrap::{Adapters, event_bus};

/// Healthy probes of the cache and the message broker, in that order.
fn healthy_probes() -> Vec<Arc<dyn HealthProbe>> {
    vec![
        Arc::new(StubProbe::healthy(Dependency::Cache)),
        Arc::new(StubProbe::healthy(Dependency::MessageBroker)),
    ]
}

/// The adapters of a node on `topic`, whose tail reads through `tail`: healthy probes, a clock at
/// the Unix epoch and identifiers counted from 1.
pub(crate) fn adapters_on(topic: &StaticTopic, tail: Box<dyn TopicTail>) -> Adapters {
    Adapters {
        probes: healthy_probes(),
        event_bus: event_bus(),
        clock: Arc::new(FixedClock::at(SystemTime::UNIX_EPOCH)),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(1)),
        topic_probe: Arc::new(topic.clone()),
        topic_tail: tail,
        topic_replay: Arc::new(topic.clone()),
    }
}

/// The adapters of a node on `topic` whose tail reads it through [`Pacing`] with `wait`
/// (`KAFKA_FETCH_MAX_WAIT`), and the count of the fetches of that tail.
pub(crate) fn paced_on(topic: &StaticTopic, wait: Duration) -> (Adapters, Arc<AtomicUsize>) {
    let (tail, fetches) = Pacing::new(topic.clone(), wait);
    (adapters_on(topic, Box::new(tail)), fetches)
}

/// A tail that awaits `wait` (`KAFKA_FETCH_MAX_WAIT`) after an empty batch, as a fetch at the end
/// of a real log does, and counts its fetches; the loop of a node on an idle `StaticTopic` behind
/// it waits instead of spinning.
#[derive(Debug)]
pub(crate) struct Pacing<T> {
    inner: T,
    wait: Duration,
    fetches: Arc<AtomicUsize>,
}

impl<T> Pacing<T> {
    /// `inner` paced with `wait`, and the count of its fetches.
    pub(crate) fn new(inner: T, wait: Duration) -> (Self, Arc<AtomicUsize>) {
        let fetches = Arc::new(AtomicUsize::new(0));
        let pacing = Self {
            inner,
            wait,
            fetches: Arc::clone(&fetches),
        };
        (pacing, fetches)
    }
}

impl<T: TopicTail> Pacing<T> {
    /// Counts the fetch, fetches from `inner` and awaits `wait` after an empty batch.
    async fn paced_fetch(&mut self) -> Result<Batch, TopicError> {
        self.fetches.fetch_add(1, Ordering::Relaxed);
        let batch = self.inner.fetch().await?;
        if batch.records.is_empty() {
            tokio::time::sleep(self.wait).await;
        }
        Ok(batch)
    }
}

#[async_trait]
impl<T: TopicTail> TopicTail for Pacing<T> {
    async fn seek(&mut self, position: Cursor) -> Result<(), TopicError> {
        self.inner.seek(position).await
    }

    async fn fetch(&mut self) -> Result<Batch, TopicError> {
        self.paced_fetch().await
    }
}

/// The value of a valid record of `event_type` whose data is `{"n":<n>}` followed by `padding`
/// bytes of a string member.
pub(crate) fn record(n: u64, event_type: &str, padding: usize) -> Vec<u8> {
    let pad = "p".repeat(padding);
    format!(
        r#"{{"id":"00000000-0000-4000-8000-{n:012x}","type":"{event_type}","occurredAt":"2026-01-01T00:00:00Z","data":{{"n":{n},"pad":"{pad}"}}}}"#
    )
    .into_bytes()
}
