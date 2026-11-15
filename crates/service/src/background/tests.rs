use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use event_stream_application::tail::TailEvents;
use event_stream_application::{
    EventPublisher, Hub, HubError, HubSettings, TailPorts, TailSettings, TailState, Window,
};
use event_stream_domain::{
    Batch, Cursor, EventType, TopicError, TopicProbe, TopicTail, Watermarks,
};
use test_support::clock::FixedClock;
use test_support::log::{LogLine, LogRecorder};
use test_support::topic::{PanickingTopicProbe, ScriptedTopicProbe, ScriptedTopicTail};
use tokio::sync::watch;
use tokio::time::{Instant, sleep, timeout};
use tracing::Level;

use super::{Background, BackgroundTaskFailed, Cause, Running, STREAM_DRAIN_TIMEOUT, after_round};

/// `KAFKA_RETRY_DELAY` of the tests.
const RETRY_DELAY: Duration = Duration::from_secs(1);

/// The adapter limit of a broker call that does not answer (`KAFKA_REQUEST_TIMEOUT`).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// The bound of every awaited call of a test that must end at once (paused time).
const LIMIT: Duration = Duration::from_secs(60);

/// The calls of the topic adapters with the time each started, since the start of the test.
#[derive(Debug, Clone)]
struct Timeline {
    start: Instant,
    entries: Arc<Mutex<Vec<(String, Duration)>>>,
}

impl Timeline {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            entries: Arc::default(),
        }
    }

    fn note(&self, call: String) {
        self.entries
            .lock()
            .unwrap()
            .push((call, self.start.elapsed()));
    }

    fn entries(&self) -> Vec<(String, Duration)> {
        self.entries.lock().unwrap().clone()
    }
}

/// The calls `(name, seconds)` the timeline must hold.
fn expected(calls: &[(&str, u64)]) -> Vec<(String, Duration)> {
    calls
        .iter()
        .map(|(call, seconds)| ((*call).to_owned(), Duration::from_secs(*seconds)))
        .collect()
}

/// A scripted probe whose calls are noted on a timeline.
#[derive(Debug)]
struct TimedProbe {
    inner: ScriptedTopicProbe,
    timeline: Timeline,
}

#[async_trait]
impl TopicProbe for TimedProbe {
    async fn partition_count(&self) -> Result<Option<u32>, TopicError> {
        self.timeline.note("partition_count".to_owned());
        self.inner.partition_count().await
    }

    async fn watermarks(&self) -> Result<Watermarks, TopicError> {
        self.timeline.note("watermarks".to_owned());
        self.inner.watermarks().await
    }
}

/// A scripted tail whose calls are noted on a timeline; a fetch first waits the next delay of
/// its script (none when the script is empty), as a broker that answers late.
#[derive(Debug)]
struct TimedTail {
    inner: ScriptedTopicTail,
    timeline: Timeline,
    delays: Arc<Mutex<VecDeque<Duration>>>,
}

#[async_trait]
impl TopicTail for TimedTail {
    async fn seek(&mut self, position: Cursor) -> Result<(), TopicError> {
        self.timeline.note(format!("seek {}", position.value()));
        self.inner.seek(position).await
    }

    async fn fetch(&mut self) -> Result<Batch, TopicError> {
        self.timeline.note("fetch".to_owned());
        let delay = self.delays.lock().unwrap().pop_front().unwrap_or_default();
        sleep(delay).await;
        self.inner.fetch().await
    }
}

/// The scripts and the timeline of a node under test.
struct Fakes {
    probe: ScriptedTopicProbe,
    tail: ScriptedTopicTail,
    delays: Arc<Mutex<VecDeque<Duration>>>,
    timeline: Timeline,
}

impl Fakes {
    fn new() -> Self {
        Self {
            probe: ScriptedTopicProbe::new(),
            tail: ScriptedTopicTail::new(),
            delays: Arc::default(),
            timeline: Timeline::new(),
        }
    }

    fn ports(&self) -> TailPorts {
        TailPorts {
            probe: Arc::new(TimedProbe {
                inner: self.probe.clone(),
                timeline: self.timeline.clone(),
            }),
            tail: Box::new(TimedTail {
                inner: self.tail.clone(),
                timeline: self.timeline.clone(),
                delays: Arc::clone(&self.delays),
            }),
            clock: Arc::new(FixedClock::at(SystemTime::UNIX_EPOCH)),
        }
    }
}

/// A node: the background work on `ports`, its hub and the receiver of its tail state.
struct Node {
    background: Background,
    hub: Arc<Hub>,
    state: watch::Receiver<TailState>,
}

fn node(ports: TailPorts) -> Node {
    let hub = Hub::new(HubSettings {
        max_subscribers: 4,
        buffer: 4,
    });
    let publisher: Arc<dyn EventPublisher> = Arc::<Hub>::clone(&hub);
    let (tail, state) = TailEvents::new(
        ports,
        Arc::new(Window::new(100, usize::MAX)),
        publisher,
        TailSettings {
            max_data_bytes: 65_536,
            start_events: 0,
        },
    );
    Node {
        background: Background {
            tail,
            hub: Arc::clone(&hub),
            retry_delay: RETRY_DELAY,
        },
        hub,
        state,
    }
}

fn unavailable() -> TopicError {
    TopicError::Unavailable("the broker refused the connection".into())
}

fn marks(earliest: u64, end: u64) -> Watermarks {
    Watermarks {
        earliest: Cursor::new(earliest),
        end: Cursor::new(end),
    }
}

fn empty(next: u64) -> Batch {
    Batch {
        records: Vec::new(),
        next: Cursor::new(next),
    }
}

fn types() -> BTreeSet<EventType> {
    BTreeSet::from([EventType::new("user.created".to_owned()).unwrap()])
}

/// Lets every task run until all of them wait (paused time advances to the next timer).
async fn settle() {
    sleep(LIMIT).await;
}

/// Runs the shutdown within [`LIMIT`] and returns how long it took.
async fn timed_shutdown(running: Running) -> Duration {
    let started = Instant::now();
    timeout(LIMIT, running.shutdown())
        .await
        .expect("the shutdown ends");
    started.elapsed()
}

fn entries(logs: &LogRecorder, message: &str) -> Vec<LogLine> {
    logs.lines()
        .into_iter()
        .filter(|line| line.message == message)
        .collect()
}

#[test]
fn a_node_waits_two_seconds_for_its_streams() {
    assert_eq!(STREAM_DRAIN_TIMEOUT, Duration::from_secs(2));
}

#[test]
fn a_failed_round_calls_for_the_retry_delay_and_a_successful_one_for_none() {
    let delay = Duration::from_millis(1_500);

    assert_eq!(after_round(Err(super::RoundFailed), delay), Some(delay));
    assert_eq!(after_round(Ok(()), delay), None);
}

#[test]
fn only_a_failed_task_is_an_error_of_the_serve_command() {
    let failed = Cause::TaskFailed("tail").result().unwrap_err();

    assert_eq!(Cause::Signal("SIGTERM").result(), Ok(()));
    assert_eq!(failed, BackgroundTaskFailed { task: "tail" });
    assert_eq!(failed.to_string(), "background task tail failed");
}

#[tokio::test(start_paused = true)]
async fn every_failed_round_is_followed_by_one_fixed_pause_and_a_successful_round_by_none() {
    let fakes = Fakes::new();
    for _ in 0..3 {
        fakes.probe.push_partition_count(Err(unavailable()));
    }
    fakes.probe.push_partition_count(Ok(Some(1)));
    fakes.probe.push_watermarks(Ok(marks(0, 0)));
    fakes.tail.push_seek(Ok(()));
    fakes.tail.push_fetch(Ok(empty(0)));
    fakes.probe.push_watermarks(Ok(marks(0, 0)));
    let node = node(fakes.ports());

    let running = node.background.spawn();
    settle().await;
    let calls = fakes.timeline.entries();
    let took = timed_shutdown(running).await;

    assert_eq!(
        calls,
        expected(&[
            ("partition_count", 0),
            ("partition_count", 1),
            ("partition_count", 2),
            ("partition_count", 3),
            ("watermarks", 3),
            ("seek 0", 3),
            ("fetch", 3),
            ("watermarks", 3),
            ("fetch", 3),
        ])
    );
    assert!(node.state.borrow().started);
    assert_eq!(took, Duration::ZERO, "the fetch in flight is cut at once");
    assert!(node.state.has_changed().is_err(), "the tail was dropped");
}

#[tokio::test(start_paused = true)]
async fn a_round_on_a_broker_that_answers_late_ends_within_its_limit_and_continues_from_p() {
    let fakes = Fakes::new();
    fakes.probe.push_partition_count(Ok(Some(1)));
    fakes.probe.push_watermarks(Ok(marks(0, 5)));
    fakes.tail.push_seek(Ok(()));
    fakes.delays.lock().unwrap().push_back(REQUEST_TIMEOUT);
    fakes.tail.push_fetch(Err(unavailable()));
    fakes.probe.push_watermarks(Ok(marks(0, 5)));
    fakes.tail.push_seek(Ok(()));
    let node = node(fakes.ports());

    let running = node.background.spawn();
    settle().await;
    let calls = fakes.timeline.entries();
    let position = node.state.borrow().position;
    let took = timed_shutdown(running).await;

    assert_eq!(
        calls,
        expected(&[
            ("partition_count", 0),
            ("watermarks", 0),
            ("seek 5", 0),
            ("fetch", 0),
            ("watermarks", 3),
            ("seek 5", 3),
            ("fetch", 4),
        ])
    );
    assert_eq!(position, Some(Cursor::new(5)));
    assert_eq!(took, Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn the_stop_flag_ends_the_pause_at_once() {
    let fakes = Fakes::new();
    fakes.probe.push_partition_count(Err(unavailable()));
    fakes.probe.push_partition_count(Err(unavailable()));
    let node = node(fakes.ports());

    let running = node.background.spawn();
    sleep(RETRY_DELAY / 2).await;
    let took = timed_shutdown(running).await;
    settle().await;

    assert_eq!(took, Duration::ZERO);
    assert_eq!(
        fakes.timeline.entries(),
        expected(&[("partition_count", 0)])
    );
    assert!(node.state.has_changed().is_err(), "the tail was dropped");
}

#[tokio::test(start_paused = true)]
async fn the_stop_flag_cuts_a_start_up_call_that_does_not_answer() {
    let fakes = Fakes::new();
    let node = node(fakes.ports());

    let running = node.background.spawn();
    settle().await;
    let took = timed_shutdown(running).await;

    assert_eq!(took, Duration::ZERO);
    assert_eq!(
        fakes.timeline.entries(),
        expected(&[("partition_count", 0)])
    );
}

#[tokio::test(start_paused = true)]
async fn a_dropped_running_ends_the_loop() {
    let fakes = Fakes::new();
    let mut node = node(fakes.ports());

    let running = node.background.spawn();
    settle().await;
    drop(running);
    let closed = timeout(LIMIT, async { while node.state.changed().await.is_ok() {} }).await;

    assert!(closed.is_ok(), "the tail is dropped with its loop");
}

#[tokio::test(start_paused = true)]
async fn a_round_that_panics_stops_the_node_with_the_failed_task() {
    let (logs, _guard) = LogRecorder::install();
    let fakes = Fakes::new();
    let mut ports = fakes.ports();
    ports.probe = Arc::new(PanickingTopicProbe);
    let node = node(ports);

    let mut running = node.background.spawn();
    let cause = timeout(LIMIT, running.stopped(std::future::pending()))
        .await
        .expect("the dead loop is seen");
    let took = timed_shutdown(running).await;

    assert_eq!(cause, Cause::TaskFailed("tail"));
    let failed = entries(&logs, "background task failed");
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].level, Level::ERROR);
    assert_eq!(failed[0].field("task"), Some("tail"));
    assert_eq!(
        entries(&logs, "shutdown signal received"),
        Vec::<LogLine>::new()
    );
    assert!(node.hub.is_closed());
    assert_eq!(took, Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn a_signal_stops_the_node_and_wins_over_a_dead_loop_that_is_ready_too() {
    let (logs, _guard) = LogRecorder::install();
    let fakes = Fakes::new();
    let mut ports = fakes.ports();
    ports.probe = Arc::new(PanickingTopicProbe);
    let node = node(ports);

    let mut running = node.background.spawn();
    settle().await;
    let cause = timeout(LIMIT, running.stopped(async { "SIGTERM" }))
        .await
        .expect("the signal is seen");

    assert_eq!(cause, Cause::Signal("SIGTERM"));
    let received = entries(&logs, "shutdown signal received");
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].level, Level::INFO);
    assert_eq!(received[0].field("signal"), Some("SIGTERM"));
    assert_eq!(
        entries(&logs, "background task failed"),
        Vec::<LogLine>::new()
    );
}

#[tokio::test(start_paused = true)]
async fn a_loop_that_ended_without_a_panic_waits_for_the_signal() {
    let fakes = Fakes::new();
    let node = node(fakes.ports());
    let mut running = node.background.spawn();
    settle().await;
    running.stop.send_replace(true);
    let started = Instant::now();

    let cause = timeout(
        LIMIT,
        running.stopped(async {
            sleep(RETRY_DELAY).await;
            "SIGINT"
        }),
    )
    .await
    .expect("the signal is seen");

    assert_eq!(cause, Cause::Signal("SIGINT"));
    assert_eq!(started.elapsed(), RETRY_DELAY);
}

#[tokio::test(start_paused = true)]
async fn the_shutdown_closes_the_hub_and_waits_for_the_streams_at_most_the_drain_timeout() {
    let fakes = Fakes::new();
    let node = node(fakes.ports());
    node.hub.start(Cursor::START);
    let mut stalled = node.hub.register(&types()).unwrap();
    let running = node.background.spawn();

    let took = timed_shutdown(running).await;

    assert_eq!(took, STREAM_DRAIN_TIMEOUT);
    assert!(node.hub.is_closed());
    assert_eq!(node.hub.register(&types()).unwrap_err(), HubError::Closed);
    assert!(timeout(LIMIT, stalled.recv()).await.unwrap().is_none());
}

#[tokio::test(start_paused = true)]
async fn the_shutdown_ends_when_the_last_stream_ends() {
    let fakes = Fakes::new();
    let node = node(fakes.ports());
    node.hub.start(Cursor::START);
    let subscriber = node.hub.register(&types()).unwrap();
    let running = node.background.spawn();
    let ending = Duration::from_millis(500);
    let stream = tokio::spawn(async move {
        sleep(ending).await;
        drop(subscriber);
    });

    let took = timed_shutdown(running).await;
    timeout(LIMIT, stream).await.unwrap().unwrap();

    assert_eq!(took, ending);
    assert_eq!(node.hub.subscriber_count(), 0);
}
