use std::collections::{BTreeMap, VecDeque};
use std::error::Error as _;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use event_stream_domain::{
    Batch, Cursor, EventType, Offset, PageLimit, Sequence, StoredEvent, TopicError, TopicProbe,
    TopicRecord, TopicTail, Watermarks,
};
use shared_domain::Clock;
use test_support::clock::FixedClock;
use test_support::fixture::stored_event;
use test_support::log::{LogLine, LogRecorder};
use test_support::static_topic::StaticTopic;
use test_support::topic::{ScriptedTopicProbe, ScriptedTopicTail};
use tokio::sync::watch;
use tokio::time::{advance, timeout};
use tracing::Level;

use super::{
    EventPublisher, RoundError, RoundFailed, TailEvents, TailPorts, TailSettings, TailState,
    TopicFault,
};
use crate::window::{Window, WindowRead};

/// The bound of every awaited call; under paused time a call that pends reaches it at once.
const LIMIT: Duration = Duration::from_secs(60);

/// The largest record value the tails of these tests accept.
const MAX_BYTES: usize = 512;

/// The second at which the clock of every harness starts.
const T0: u64 = 1_700_000_000;

/// The time `after` seconds after `T0`.
fn time(after: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(T0))
        .and_then(|start| start.checked_add(Duration::from_secs(after)))
        .unwrap()
}

/// One list of the calls made to the probe and to the tail, in the order they were made.
#[derive(Debug, Clone, Default)]
struct Journal(Arc<Mutex<Vec<String>>>);

impl Journal {
    fn note(&self, call: String) {
        self.0.lock().unwrap().push(call);
    }

    /// The calls since the last `take`, which forgets them.
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

/// A scripted probe whose calls enter the journal.
#[derive(Debug)]
struct JournaledProbe {
    inner: ScriptedTopicProbe,
    journal: Journal,
}

#[async_trait]
impl TopicProbe for JournaledProbe {
    async fn partition_count(&self) -> Result<Option<u32>, TopicError> {
        self.journal.note("partition count".to_owned());
        self.inner.partition_count().await
    }

    async fn watermarks(&self) -> Result<Watermarks, TopicError> {
        self.journal.note("watermarks".to_owned());
        self.inner.watermarks().await
    }
}

/// A scripted tail whose calls enter the journal.
#[derive(Debug)]
struct JournaledTail {
    inner: ScriptedTopicTail,
    journal: Journal,
}

#[async_trait]
impl TopicTail for JournaledTail {
    async fn seek(&mut self, position: Cursor) -> Result<(), TopicError> {
        self.journal.note(format!("seek {position}"));
        self.inner.seek(position).await
    }

    async fn fetch(&mut self) -> Result<Batch, TopicError> {
        self.journal.note("fetch".to_owned());
        self.inner.fetch().await
    }
}

/// A clock that the test moves by hand, so that a time the tail did not record again shows.
#[derive(Debug, Clone)]
struct ManualClock(Arc<Mutex<SystemTime>>);

impl ManualClock {
    /// A clock at `after` seconds after `T0`.
    fn at(after: u64) -> Self {
        Self(Arc::new(Mutex::new(time(after))))
    }

    /// Moves the clock to `after` seconds after `T0`.
    fn set(&self, after: u64) {
        *self.0.lock().unwrap() = time(after);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self.0.lock().unwrap()
    }
}

/// Records the calls it receives; `publish` answers the next scripted count of subscribers that
/// fell behind (0 once the script is empty) and, when it watches a window, records the newest
/// event the window held at that moment.
#[derive(Debug, Default)]
struct RecordingPublisher {
    calls: Mutex<Vec<String>>,
    behind: Mutex<VecDeque<usize>>,
    window: Option<Arc<Window>>,
}

impl RecordingPublisher {
    fn watching(window: Arc<Window>) -> Self {
        Self {
            window: Some(window),
            ..Self::default()
        }
    }

    fn falls_behind(&self, counts: &[usize]) {
        self.behind.lock().unwrap().extend(counts);
    }

    /// The calls since the last `take`, which forgets them.
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

impl EventPublisher for RecordingPublisher {
    fn started(&self, position: Cursor) {
        self.record(format!("started {position}"));
    }

    fn publish(&self, event: &StoredEvent) -> usize {
        let call = self.window.as_ref().map_or_else(
            || format!("publish {}", event.sequence),
            |window| {
                let last = window.last().map_or(0, Sequence::value);
                format!("publish {} held {last}", event.sequence)
            },
        );
        self.record(call);
        self.behind.lock().unwrap().pop_front().unwrap_or(0)
    }

    fn advance(&self, position: Cursor) {
        self.record(format!("advance {position}"));
    }
}

/// A tail on scripted ports, a manual clock, a real window and a recording publisher.
#[derive(Debug)]
struct Harness {
    probe: ScriptedTopicProbe,
    tail: ScriptedTopicTail,
    journal: Journal,
    clock: ManualClock,
    window: Arc<Window>,
    publisher: Arc<RecordingPublisher>,
    events: TailEvents,
    state: watch::Receiver<TailState>,
}

impl Harness {
    fn new(start_events: u64) -> Self {
        let probe = ScriptedTopicProbe::new();
        let tail = ScriptedTopicTail::new();
        let journal = Journal::default();
        let clock = ManualClock::at(0);
        let window = Arc::new(Window::new(100, 1_000_000));
        let publisher = Arc::new(RecordingPublisher::watching(Arc::clone(&window)));
        let ports = TailPorts {
            probe: Arc::new(JournaledProbe {
                inner: probe.clone(),
                journal: journal.clone(),
            }),
            tail: Box::new(JournaledTail {
                inner: tail.clone(),
                journal: journal.clone(),
            }),
            clock: Arc::new(clock.clone()),
        };
        let settings = TailSettings {
            max_data_bytes: MAX_BYTES,
            start_events,
        };
        let shared: Arc<dyn EventPublisher> = Arc::<RecordingPublisher>::clone(&publisher);
        let (events, state) = TailEvents::new(ports, Arc::clone(&window), shared, settings);
        Self {
            probe,
            tail,
            journal,
            clock,
            window,
            publisher,
            events,
            state,
        }
    }

    async fn round(&mut self) -> Result<(), RoundFailed> {
        timeout(LIMIT, self.events.round())
            .await
            .expect("the round ends")
    }

    /// The state last published on the watch channel.
    fn state(&self) -> TailState {
        self.state.borrow().clone()
    }

    /// The state published since the last call; fails when nothing was published.
    fn published(&mut self) -> TailState {
        assert!(self.state.has_changed().unwrap(), "a state was published");
        self.state.borrow_and_update().clone()
    }

    /// Scripts a start-up that passes: one partition, the watermarks and the first seek.
    fn passes_start_up(&self, earliest: u64, end: u64) {
        self.probe.push_partition_count(Ok(Some(1)));
        self.probe.push_watermarks(Ok(watermarks(earliest, end)));
        self.tail.push_seek(Ok(()));
    }

    /// Runs a start-up that passes and forgets its calls of the ports and of the publisher.
    async fn start(&mut self, earliest: u64, end: u64) {
        self.passes_start_up(earliest, end);
        assert_eq!(self.round().await, Ok(()));
        self.journal.take();
        self.publisher.take();
    }

    /// Scripts a fetch that answers `batch`.
    fn fetches(&self, records: Vec<TopicRecord>, next: u64) {
        self.tail.push_fetch(Ok(Batch {
            records,
            next: Cursor::new(next),
        }));
    }

    /// Scripts a round that fails in every step: the fetch, the watermarks and the re-seek.
    fn fails_every_step(&self) {
        self.tail.push_fetch(Err(unavailable("broker gone")));
        self.probe.push_watermarks(Err(unavailable("broker gone")));
        self.tail.push_seek(Ok(()));
    }
}

fn watermarks(earliest: u64, end: u64) -> Watermarks {
    Watermarks {
        earliest: Cursor::new(earliest),
        end: Cursor::new(end),
    }
}

fn unavailable(text: &str) -> TopicError {
    TopicError::Unavailable(text.into())
}

fn envelope(offset: u64, data: &str) -> Vec<u8> {
    format!(
        r#"{{"id":"00000000-0000-0000-0000-{offset:012}","type":"order.paid","occurredAt":"2026-01-01T00:00:00Z","data":{data}}}"#
    )
    .into_bytes()
}

fn valid(offset: u64) -> TopicRecord {
    TopicRecord {
        offset: Offset::new(offset),
        value: Some(envelope(offset, "{}")),
    }
}

fn valid_range(offsets: Range<u64>) -> Vec<TopicRecord> {
    offsets.map(valid).collect()
}

fn invalid(offset: u64) -> TopicRecord {
    TopicRecord {
        offset: Offset::new(offset),
        value: Some(b"not json".to_vec()),
    }
}

fn without_value(offset: u64) -> TopicRecord {
    TopicRecord {
        offset: Offset::new(offset),
        value: None,
    }
}

fn too_large(offset: u64) -> TopicRecord {
    let data = format!("\"{}\"", "x".repeat(MAX_BYTES));
    TopicRecord {
        offset: Offset::new(offset),
        value: Some(envelope(offset, &data)),
    }
}

/// The ids of the events the window holds, oldest first.
fn held(window: &Window) -> Vec<u64> {
    let types = BTreeMap::from([(
        EventType::new("order.paid".to_owned()).unwrap(),
        Cursor::START,
    )]);
    let limit = PageLimit {
        rows: usize::MAX,
        bytes: usize::MAX,
    };
    match window.read(window.start(), Cursor::new(u64::MAX), &types, limit) {
        WindowRead::Page { events, .. } => {
            events.iter().map(|event| event.sequence.value()).collect()
        }
        WindowRead::Evicted => panic!("a read from the floor of the window is never evicted"),
    }
}

fn messages(logs: &LogRecorder) -> Vec<String> {
    logs.lines().into_iter().map(|line| line.message).collect()
}

fn entries(logs: &LogRecorder, message: &str) -> Vec<LogLine> {
    logs.lines()
        .into_iter()
        .filter(|line| line.message == message)
        .collect()
}

fn fields(line: &LogLine) -> Vec<(&str, &str)> {
    line.fields
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect()
}

fn assert_send<T: Send>(_value: &T) {}

#[test]
fn the_topic_faults_have_their_texts() {
    assert_eq!(
        TopicFault::Unreachable.to_string(),
        "the topic is unreachable"
    );
    assert_eq!(TopicFault::Missing.to_string(), "the topic does not exist");
    assert_eq!(
        TopicFault::Partitions(3).to_string(),
        "the topic must have exactly one partition"
    );
    assert!(TopicFault::Missing.source().is_none());
}

#[test]
fn a_failed_round_has_its_text() {
    assert_eq!(RoundFailed.to_string(), "the round failed");
}

#[test]
fn a_round_error_reads_as_its_cause() {
    let read = RoundError::from(unavailable("connection refused"));
    let fault = RoundError::Fault(TopicFault::Missing);

    assert_eq!(read.to_string(), "the topic is unavailable");
    assert_eq!(
        read.source().map(ToString::to_string),
        Some("connection refused".to_owned())
    );
    assert_eq!(fault.to_string(), "the topic does not exist");
    assert!(fault.source().is_none());
}

#[test]
fn the_default_tail_state_knows_nothing_yet() {
    let state = TailState::default();

    assert_eq!(state.fault, None);
    assert!(!state.started);
    assert_eq!(state.position, None);
    assert_eq!(state.earliest, None);
    assert_eq!(state.end, None);
    assert!(!state.broker_reachable);
    assert_eq!(state.observations, 0);
    assert_eq!(state.observed_at, None);
    assert_eq!(state.caught_up_at, None);
    assert_eq!(state.rejected, 0);
    assert_eq!(state.retention_losses, 0);
}

#[test]
fn the_publisher_is_used_through_a_shared_trait_object() {
    let recording = Arc::new(RecordingPublisher::default());
    recording.falls_behind(&[2]);
    let publisher: Arc<dyn EventPublisher> = Arc::<RecordingPublisher>::clone(&recording);

    publisher.started(Cursor::new(4));
    let behind = publisher.publish(&stored_event(5, "order.paid", "{}"));
    publisher.advance(Cursor::new(7));

    assert_eq!(behind, 2);
    assert_eq!(
        recording.take(),
        vec!["started 4", "publish 5", "advance 7"]
    );
}

#[test]
fn the_ports_hold_the_topic_readers_and_the_clock() {
    let now = time(0);
    let topic = StaticTopic::empty();
    let ports = TailPorts {
        probe: Arc::new(topic.clone()),
        tail: Box::new(topic),
        clock: Arc::new(FixedClock::at(now)),
    };

    assert_eq!(ports.clock.now(), now);
}

#[test]
fn the_settings_are_copied_values() {
    let settings = TailSettings {
        max_data_bytes: 65_536,
        start_events: 1_000,
    };
    let copy = settings;

    assert_eq!(copy, settings);
    assert_eq!(copy.max_data_bytes, 65_536);
    assert_eq!(copy.start_events, 1_000);
}

#[test]
fn the_watermarks_are_read_every_second() {
    assert_eq!(TailEvents::OBSERVE_EVERY, Duration::from_secs(1));
}

#[test]
fn a_new_tail_publishes_the_default_state() {
    let harness = Harness::new(0);

    assert_eq!(harness.state(), TailState::default());
    assert!(!harness.state.has_changed().unwrap());
}

#[test]
fn the_round_can_run_on_another_thread() {
    let mut harness = Harness::new(0);
    let round = harness.events.round();

    assert_send(&round);
}

#[tokio::test(start_paused = true)]
async fn an_unreachable_broker_fails_every_start_up_round_and_is_logged_once() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(100_000);
    harness
        .probe
        .push_partition_count(Err(unavailable("connection refused")));
    harness
        .probe
        .push_partition_count(Err(unavailable("connection refused")));

    for _ in 0..2 {
        assert_eq!(harness.round().await, Err(RoundFailed));
        let state = harness.published();
        assert_eq!(state.fault, Some(TopicFault::Unreachable));
        assert!(!state.broker_reachable);
        assert_eq!(state.position, None);
    }

    assert_eq!(
        harness.journal.take(),
        vec!["partition count", "partition count"]
    );
    let lines = logs.lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].level, Level::WARN);
    assert_eq!(lines[0].message, "event topic unreachable");
    assert_eq!(
        fields(&lines[0]),
        vec![("error", "the topic is unavailable: connection refused")]
    );
    assert_eq!(harness.publisher.take(), Vec::<String>::new());
}

#[tokio::test(start_paused = true)]
async fn a_missing_topic_and_a_topic_of_three_partitions_are_logged_once_per_change() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(100_000);
    harness.probe.push_partition_count(Ok(None));
    harness.probe.push_partition_count(Ok(Some(3)));
    harness.probe.push_partition_count(Ok(Some(3)));

    let mut faults = Vec::new();
    for _ in 0..3 {
        assert_eq!(harness.round().await, Err(RoundFailed));
        faults.push(harness.published().fault);
    }

    assert_eq!(
        faults,
        vec![
            Some(TopicFault::Missing),
            Some(TopicFault::Partitions(3)),
            Some(TopicFault::Partitions(3)),
        ]
    );
    assert_eq!(
        harness.journal.take(),
        vec!["partition count", "partition count", "partition count"]
    );
    let lines = logs.lines();
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|line| line.level == Level::ERROR));
    assert!(
        lines
            .iter()
            .all(|line| line.message == "event topic invalid")
    );
    assert_eq!(fields(&lines[0]), vec![("reason", "topic-not-found")]);
    assert_eq!(fields(&lines[1]), vec![("reason", "partition-count")]);
    let state = harness.state();
    assert!(!state.broker_reachable);
    assert_eq!(state.observations, 0);
    assert_eq!(state.position, None);
}

#[tokio::test(start_paused = true)]
async fn a_fault_is_logged_again_once_another_fault_came_between() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(0);
    harness.probe.push_partition_count(Ok(None));
    harness
        .probe
        .push_partition_count(Err(unavailable("connection refused")));
    harness.probe.push_partition_count(Ok(None));

    for _ in 0..3 {
        assert_eq!(harness.round().await, Err(RoundFailed));
    }
    harness.passes_start_up(0, 0);
    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(
        messages(&logs),
        vec![
            "event topic invalid",
            "event topic unreachable",
            "event topic invalid",
            "event tail ready",
        ]
    );
    assert_eq!(harness.state().fault, None);
}

#[tokio::test(start_paused = true)]
async fn a_failed_start_up_watermark_read_makes_the_topic_unreachable() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(0);
    harness.probe.push_partition_count(Ok(Some(1)));
    harness
        .probe
        .push_watermarks(Err(unavailable("request timed out")));

    assert_eq!(harness.round().await, Err(RoundFailed));

    assert_eq!(
        harness.journal.take(),
        vec!["partition count", "watermarks"]
    );
    let state = harness.published();
    assert_eq!(state.fault, Some(TopicFault::Unreachable));
    assert!(!state.broker_reachable);
    assert_eq!(state.observations, 0);
    assert_eq!(state.observed_at, None);
    assert_eq!(state.position, None);
    let lines = entries(&logs, "event topic unreachable");
    assert_eq!(lines.len(), 1);
    assert_eq!(
        fields(&lines[0]),
        vec![("error", "the topic is unavailable: request timed out")]
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_first_seek_makes_the_topic_unreachable_until_a_seek_passes() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(100_000);
    harness.probe.push_partition_count(Ok(Some(1)));
    harness.probe.push_watermarks(Ok(watermarks(20, 50)));
    harness.tail.push_seek(Err(unavailable("seek refused")));

    assert_eq!(harness.round().await, Err(RoundFailed));

    assert_eq!(
        harness.journal.take(),
        vec!["partition count", "watermarks", "seek 20"]
    );
    let state = harness.published();
    assert_eq!(state.fault, Some(TopicFault::Unreachable));
    assert!(!state.broker_reachable);
    assert_eq!(state.observations, 1);
    assert_eq!(state.position, None);
    assert_eq!(harness.window.start(), Cursor::new(20));
    assert_eq!(
        fields(&entries(&logs, "event topic unreachable")[0]),
        vec![("error", "the topic is unavailable: seek refused")]
    );

    harness.passes_start_up(20, 50);
    assert_eq!(harness.round().await, Ok(()));

    let state = harness.published();
    assert_eq!(state.fault, None);
    assert!(state.broker_reachable);
    assert_eq!(state.observations, 2);
    assert_eq!(state.position, Some(Cursor::new(20)));
    assert_eq!(messages(&logs), vec!["event topic unreachable"]);
}

#[tokio::test(start_paused = true)]
async fn the_start_position_is_start_events_below_the_end() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(100_000);
    harness
        .window
        .push(Arc::new(stored_event(7, "order.paid", "{}")));
    harness.passes_start_up(100, 200_100);

    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(
        harness.journal.take(),
        vec!["partition count", "watermarks", "seek 100100"]
    );
    assert_eq!(harness.window.start(), Cursor::new(100_100));
    assert_eq!(harness.window.last(), None);
    assert_eq!(
        harness.published(),
        TailState {
            fault: None,
            started: false,
            position: Some(Cursor::new(100_100)),
            earliest: Some(Cursor::new(100)),
            end: Some(Cursor::new(200_100)),
            broker_reachable: true,
            observations: 1,
            observed_at: Some(time(0)),
            caught_up_at: None,
            rejected: 0,
            retention_losses: 0,
        }
    );
    assert_eq!(harness.publisher.take(), Vec::<String>::new());
    assert_eq!(logs.lines(), Vec::<LogLine>::new());
}

#[tokio::test(start_paused = true)]
async fn the_start_position_is_never_below_the_log_start() {
    let mut harness = Harness::new(100_000);
    harness.passes_start_up(100, 5_000);

    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(
        harness.journal.take(),
        vec!["partition count", "watermarks", "seek 100"]
    );
    assert_eq!(harness.state().position, Some(Cursor::new(100)));
}

#[tokio::test(start_paused = true)]
async fn an_empty_topic_makes_the_node_ready_at_once() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(100_000);
    harness.passes_start_up(42, 42);

    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(harness.publisher.take(), vec!["started 42"]);
    let state = harness.published();
    assert!(state.started);
    assert_eq!(state.position, Some(Cursor::new(42)));
    assert_eq!(state.caught_up_at, Some(time(0)));
    let lines = logs.lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].level, Level::INFO);
    assert_eq!(lines[0].message, "event tail ready");
    assert_eq!(fields(&lines[0]), vec![("position", "42")]);
}

#[tokio::test(start_paused = true)]
async fn no_start_events_make_the_node_ready_at_the_end() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(0);
    harness.passes_start_up(0, 500);

    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(
        harness.journal.take(),
        vec!["partition count", "watermarks", "seek 500"]
    );
    assert_eq!(harness.publisher.take(), vec!["started 500"]);
    assert!(harness.state().started);
    assert_eq!(
        fields(&entries(&logs, "event tail ready")[0]),
        vec![("position", "500")]
    );
}

#[tokio::test(start_paused = true)]
async fn one_start_event_keeps_the_node_starting_below_the_end() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(1);
    harness.passes_start_up(0, 500);

    assert_eq!(harness.round().await, Ok(()));

    let state = harness.state();
    assert_eq!(state.position, Some(Cursor::new(499)));
    assert!(!state.started);
    assert_eq!(state.caught_up_at, None);
    assert_eq!(harness.publisher.take(), Vec::<String>::new());
    assert_eq!(logs.lines(), Vec::<LogLine>::new());
}

#[tokio::test(start_paused = true)]
async fn the_window_fills_without_publishing_until_the_node_reaches_its_goal() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(3);
    harness.start(0, 10).await;

    harness.fetches(valid_range(7..9), 9);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(held(&harness.window), vec![8, 9]);
    assert_eq!(harness.publisher.take(), vec!["advance 9"]);
    assert!(!harness.state().started);
    assert_eq!(harness.state().caught_up_at, None);

    harness.clock.set(5);
    harness.fetches(valid_range(9..10), 10);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(held(&harness.window), vec![8, 9, 10]);
    assert_eq!(harness.publisher.take(), vec!["advance 10", "started 10"]);
    let state = harness.state();
    assert!(state.started);
    assert_eq!(state.caught_up_at, Some(time(5)));

    harness.clock.set(6);
    harness.fetches(valid_range(10..12), 12);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(held(&harness.window), vec![8, 9, 10, 11, 12]);
    assert_eq!(
        harness.publisher.take(),
        vec!["publish 11 held 11", "publish 12 held 12", "advance 12"]
    );
    assert_eq!(harness.state().caught_up_at, Some(time(6)));
    assert_eq!(harness.journal.take(), vec!["fetch", "fetch", "fetch"]);
    let ready = entries(&logs, "event tail ready");
    assert_eq!(ready.len(), 1);
    assert_eq!(fields(&ready[0]), vec![("position", "10")]);
}

#[tokio::test(start_paused = true)]
async fn a_higher_end_moves_the_goal_and_caught_up_waits_for_a_batch_at_the_goal() {
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;
    assert_eq!(harness.state().caught_up_at, Some(time(0)));

    harness.clock.set(1);
    harness.fetches(Vec::new(), 10);
    harness.probe.push_watermarks(Ok(watermarks(0, 20)));
    harness.tail.push_seek(Ok(()));
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(
        harness.journal.take(),
        vec!["fetch", "watermarks", "seek 10"]
    );
    let state = harness.state();
    assert_eq!(state.end, Some(Cursor::new(20)));
    assert_eq!(state.caught_up_at, Some(time(1)));

    harness.clock.set(2);
    harness.fetches(valid_range(10..15), 15);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.state().caught_up_at, Some(time(1)));

    harness.clock.set(3);
    harness.fetches(valid_range(15..19), 19);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.state().caught_up_at, Some(time(1)));

    harness.clock.set(4);
    harness.fetches(valid_range(19..20), 20);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.state().caught_up_at, Some(time(4)));
    assert_eq!(harness.journal.take(), vec!["fetch", "fetch", "fetch"]);
}

#[tokio::test(start_paused = true)]
async fn a_batch_publishes_valid_events_and_skips_invalid_records() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(0);
    harness.start(0, 0).await;
    harness.fetches(
        vec![
            valid(0),
            invalid(1),
            without_value(2),
            too_large(3),
            valid(4),
        ],
        5,
    );

    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(held(&harness.window), vec![1, 5]);
    assert_eq!(
        harness.publisher.take(),
        vec!["publish 1 held 1", "publish 5 held 5", "advance 5"]
    );
    let state = harness.published();
    assert_eq!(state.rejected, 3);
    assert_eq!(state.position, Some(Cursor::new(5)));
    let rejected = entries(&logs, "event record rejected");
    assert!(rejected.iter().all(|line| line.level == Level::WARN));
    assert_eq!(
        rejected.iter().map(fields).collect::<Vec<_>>(),
        vec![
            vec![
                ("offset", "1"),
                ("reason", "not a JSON object with the envelope members")
            ],
            vec![("offset", "2"), ("reason", "missing value")],
            vec![("offset", "3"), ("reason", "value too large")],
        ]
    );
    assert_eq!(
        messages(&logs),
        vec![
            "event tail ready",
            "event record rejected",
            "event record rejected",
            "event record rejected",
        ]
    );

    harness.fetches(vec![invalid(5)], 6);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.state().rejected, 4);
}

#[tokio::test(start_paused = true)]
async fn subscribers_that_fell_behind_are_logged_once_per_batch() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(0);
    harness.start(0, 0).await;
    harness.publisher.falls_behind(&[1, 0, 2]);
    harness.fetches(valid_range(0..3), 3);

    assert_eq!(harness.round().await, Ok(()));

    let behind = entries(&logs, "subscribers fell behind");
    assert_eq!(behind.len(), 1);
    assert_eq!(behind[0].level, Level::WARN);
    assert_eq!(fields(&behind[0]), vec![("count", "3")]);

    harness.fetches(valid_range(3..5), 5);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(entries(&logs, "subscribers fell behind").len(), 1);
}

#[tokio::test(start_paused = true)]
async fn retention_moves_the_position_to_the_log_start() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(0);
    harness.start(0, 300).await;
    harness.clock.set(1);
    harness.tail.push_fetch(Err(TopicError::BelowEarliest {
        earliest: Cursor::new(500),
    }));
    harness.tail.push_seek(Ok(()));
    harness.probe.push_watermarks(Ok(watermarks(500, 600)));

    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(
        harness.journal.take(),
        vec!["fetch", "seek 500", "watermarks"]
    );
    assert_eq!(harness.publisher.take(), vec!["advance 500"]);
    let state = harness.published();
    assert_eq!(state.retention_losses, 1);
    assert_eq!(state.position, Some(Cursor::new(500)));
    assert_eq!(state.earliest, Some(Cursor::new(500)));
    assert_eq!(state.end, Some(Cursor::new(600)));
    assert_eq!(state.caught_up_at, Some(time(0)));
    let lost = entries(&logs, "events lost to retention");
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0].level, Level::ERROR);
    assert_eq!(
        fields(&lost[0]),
        vec![("from", "300"), ("to", "500"), ("distance", "200")]
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_seek_to_the_log_start_fails_the_round() {
    let mut harness = Harness::new(0);
    harness.start(0, 300).await;
    harness.tail.push_fetch(Err(TopicError::BelowEarliest {
        earliest: Cursor::new(500),
    }));
    harness.tail.push_seek(Err(unavailable("seek refused")));
    harness.probe.push_watermarks(Ok(watermarks(500, 600)));

    assert_eq!(harness.round().await, Err(RoundFailed));

    assert_eq!(
        harness.journal.take(),
        vec!["fetch", "seek 500", "watermarks"]
    );
    assert_eq!(harness.publisher.take(), vec!["advance 500"]);
    let state = harness.published();
    assert_eq!(state.retention_losses, 1);
    assert_eq!(state.position, Some(Cursor::new(500)));
}

#[tokio::test(start_paused = true)]
async fn an_unavailable_fetch_fails_the_round_and_re_seeks_after_the_watermarks() {
    let mut harness = Harness::new(0);
    harness.start(0, 300).await;
    harness
        .tail
        .push_fetch(Err(unavailable("connection reset")));
    harness.probe.push_watermarks(Ok(watermarks(0, 300)));
    harness.tail.push_seek(Ok(()));

    assert_eq!(harness.round().await, Err(RoundFailed));

    assert_eq!(
        harness.journal.take(),
        vec!["fetch", "watermarks", "seek 300"]
    );
    let state = harness.published();
    assert_eq!(state.position, Some(Cursor::new(300)));
    assert!(state.broker_reachable);
}

#[tokio::test(start_paused = true)]
async fn an_unavailable_fetch_re_seeks_after_a_failed_watermark_read_too() {
    let mut harness = Harness::new(0);
    harness.start(0, 300).await;
    harness.fails_every_step();

    assert_eq!(harness.round().await, Err(RoundFailed));

    assert_eq!(
        harness.journal.take(),
        vec!["fetch", "watermarks", "seek 300"]
    );
}

#[tokio::test(start_paused = true)]
async fn a_fetch_that_fails_in_every_round_moves_nothing() {
    let mut harness = Harness::new(0);
    harness.start(0, 300).await;
    harness.clock.set(5);

    for _ in 0..3 {
        harness
            .tail
            .push_fetch(Err(unavailable("corrupt record batch")));
        harness.probe.push_watermarks(Ok(watermarks(0, 310)));
        harness.tail.push_seek(Ok(()));
        assert_eq!(harness.round().await, Err(RoundFailed));
    }

    assert_eq!(
        harness.journal.take(),
        vec![
            "fetch",
            "watermarks",
            "seek 300",
            "fetch",
            "watermarks",
            "seek 300",
            "fetch",
            "watermarks",
            "seek 300",
        ]
    );
    let state = harness.state();
    assert_eq!(state.position, Some(Cursor::new(300)));
    assert_eq!(state.rejected, 0);
    assert_eq!(state.caught_up_at, Some(time(0)));
    assert_eq!(held(&harness.window), Vec::<u64>::new());
    assert_eq!(harness.publisher.take(), Vec::<String>::new());
}

#[tokio::test(start_paused = true)]
async fn an_empty_fetch_re_seeks_when_the_end_is_above_the_position() {
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;
    harness.fetches(Vec::new(), 10);
    harness.probe.push_watermarks(Ok(watermarks(0, 11)));
    harness.tail.push_seek(Ok(()));

    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(
        harness.journal.take(),
        vec!["fetch", "watermarks", "seek 10"]
    );
    assert_eq!(harness.publisher.take(), vec!["advance 10"]);
}

#[tokio::test(start_paused = true)]
async fn an_empty_fetch_at_the_end_does_not_re_seek() {
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;
    harness.fetches(Vec::new(), 10);
    harness.probe.push_watermarks(Ok(watermarks(0, 10)));

    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(harness.journal.take(), vec!["fetch", "watermarks"]);
}

#[tokio::test(start_paused = true)]
async fn a_failed_re_seek_fails_the_round() {
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;
    harness.fetches(Vec::new(), 10);
    harness.probe.push_watermarks(Ok(watermarks(0, 20)));
    harness.tail.push_seek(Err(unavailable("seek refused")));

    assert_eq!(harness.round().await, Err(RoundFailed));

    assert_eq!(
        harness.journal.take(),
        vec!["fetch", "watermarks", "seek 10"]
    );
}

#[tokio::test(start_paused = true)]
async fn a_fetch_with_records_does_not_re_seek_when_the_end_is_above_the_position() {
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;
    advance(TailEvents::OBSERVE_EVERY).await;
    harness.fetches(valid_range(10..11), 11);
    harness.probe.push_watermarks(Ok(watermarks(0, 20)));

    assert_eq!(harness.round().await, Ok(()));

    assert_eq!(harness.journal.take(), vec!["fetch", "watermarks"]);
}

#[tokio::test(start_paused = true)]
async fn a_successful_watermark_read_after_a_failed_one_re_seeks() {
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;
    harness.fetches(Vec::new(), 10);
    harness
        .probe
        .push_watermarks(Err(unavailable("connection refused")));
    assert_eq!(harness.round().await, Err(RoundFailed));
    assert_eq!(harness.journal.take(), vec!["fetch", "watermarks"]);

    harness.fetches(Vec::new(), 10);
    harness.probe.push_watermarks(Ok(watermarks(0, 10)));
    harness.tail.push_seek(Ok(()));
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(
        harness.journal.take(),
        vec!["fetch", "watermarks", "seek 10"]
    );

    harness.fetches(Vec::new(), 10);
    harness
        .probe
        .push_watermarks(Err(unavailable("connection refused")));
    assert_eq!(harness.round().await, Err(RoundFailed));
    advance(TailEvents::OBSERVE_EVERY).await;
    harness.fetches(valid_range(10..11), 11);
    harness.probe.push_watermarks(Ok(watermarks(0, 11)));
    harness.tail.push_seek(Ok(()));
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(
        harness.journal.take(),
        vec!["fetch", "watermarks", "fetch", "watermarks", "seek 11"]
    );
}

#[tokio::test(start_paused = true)]
async fn a_fetch_with_records_reads_the_watermarks_once_the_last_attempt_is_a_second_old() {
    let mut harness = Harness::new(0);
    harness.start(0, 0).await;

    advance(Duration::from_millis(999)).await;
    harness.fetches(valid_range(0..1), 1);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.journal.take(), vec!["fetch"]);

    advance(Duration::from_millis(1)).await;
    harness.fetches(valid_range(1..2), 2);
    harness.probe.push_watermarks(Ok(watermarks(0, 2)));
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.journal.take(), vec!["fetch", "watermarks"]);

    advance(Duration::from_millis(999)).await;
    harness.fetches(valid_range(2..3), 3);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.journal.take(), vec!["fetch"]);

    advance(Duration::from_millis(1)).await;
    harness.fetches(valid_range(3..4), 4);
    harness
        .probe
        .push_watermarks(Err(unavailable("connection refused")));
    assert_eq!(harness.round().await, Err(RoundFailed));
    assert_eq!(harness.journal.take(), vec!["fetch", "watermarks"]);

    advance(Duration::from_millis(999)).await;
    harness.fetches(valid_range(4..5), 5);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.journal.take(), vec!["fetch"]);
}

#[tokio::test(start_paused = true)]
async fn a_failed_watermark_read_keeps_the_last_observation() {
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;

    harness.clock.set(1);
    harness.fetches(Vec::new(), 10);
    harness
        .probe
        .push_watermarks(Err(unavailable("connection refused")));
    assert_eq!(harness.round().await, Err(RoundFailed));
    let state = harness.published();
    assert!(!state.broker_reachable);
    assert_eq!(state.observations, 1);
    assert_eq!(state.observed_at, Some(time(0)));
    assert_eq!(state.earliest, Some(Cursor::new(0)));
    assert_eq!(state.end, Some(Cursor::new(10)));

    harness.clock.set(2);
    harness.fetches(Vec::new(), 10);
    harness.probe.push_watermarks(Ok(watermarks(3, 12)));
    harness.tail.push_seek(Ok(()));
    assert_eq!(harness.round().await, Ok(()));
    let state = harness.published();
    assert!(state.broker_reachable);
    assert_eq!(state.observations, 2);
    assert_eq!(state.observed_at, Some(time(2)));
    assert_eq!(state.earliest, Some(Cursor::new(3)));
    assert_eq!(state.end, Some(Cursor::new(12)));
    assert_eq!(state.caught_up_at, Some(time(2)));

    harness.clock.set(3);
    harness.fetches(valid_range(10..11), 11);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.state().caught_up_at, Some(time(2)));

    harness.clock.set(4);
    harness.fetches(valid_range(11..12), 12);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(harness.state().caught_up_at, Some(time(4)));
}

#[tokio::test(start_paused = true)]
async fn the_tail_logs_failed_and_recovered_only_at_the_transitions() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;

    harness.tail.push_fetch(Err(unavailable("first")));
    harness.probe.push_watermarks(Err(unavailable("second")));
    harness.tail.push_seek(Err(unavailable("third")));
    assert_eq!(harness.round().await, Err(RoundFailed));
    for _ in 0..2 {
        harness.fails_every_step();
        assert_eq!(harness.round().await, Err(RoundFailed));
    }
    let failed = entries(&logs, "event tail failed");
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].level, Level::WARN);
    assert_eq!(
        fields(&failed[0]),
        vec![("error", "the topic is unavailable: first")]
    );

    harness.fetches(Vec::new(), 10);
    harness.probe.push_watermarks(Ok(watermarks(0, 10)));
    harness.tail.push_seek(Ok(()));
    assert_eq!(harness.round().await, Ok(()));
    harness.fetches(Vec::new(), 10);
    harness.probe.push_watermarks(Ok(watermarks(0, 10)));
    assert_eq!(harness.round().await, Ok(()));

    let recovered = entries(&logs, "event tail recovered");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].level, Level::INFO);
    assert_eq!(fields(&recovered[0]), vec![("position", "10")]);
    assert_eq!(
        messages(&logs),
        vec![
            "event tail ready",
            "event tail failed",
            "event tail recovered"
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_watermark_read_alone_fails_the_round_and_names_its_error() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;
    harness.fetches(Vec::new(), 10);
    harness.probe.push_watermarks(Err(unavailable("second")));

    assert_eq!(harness.round().await, Err(RoundFailed));

    assert_eq!(
        fields(&entries(&logs, "event tail failed")[0]),
        vec![("error", "the topic is unavailable: second")]
    );
}

#[tokio::test(start_paused = true)]
async fn a_node_that_never_succeeded_logs_neither_failed_nor_recovered() {
    let (logs, _guard) = LogRecorder::install();
    let mut harness = Harness::new(0);
    for _ in 0..2 {
        harness
            .probe
            .push_partition_count(Err(unavailable("connection refused")));
        assert_eq!(harness.round().await, Err(RoundFailed));
    }
    harness.passes_start_up(0, 0);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(
        messages(&logs),
        vec!["event topic unreachable", "event tail ready"]
    );

    harness.fails_every_step();
    assert_eq!(harness.round().await, Err(RoundFailed));

    assert_eq!(
        messages(&logs),
        vec![
            "event topic unreachable",
            "event tail ready",
            "event tail failed",
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_started_node_stays_started_while_the_broker_is_away() {
    let mut harness = Harness::new(0);
    harness.start(0, 10).await;

    for second in 1..=2 {
        harness.clock.set(second);
        harness.fails_every_step();
        assert_eq!(harness.round().await, Err(RoundFailed));
        let state = harness.published();
        assert!(state.started);
        assert!(!state.broker_reachable);
        assert_eq!(state.observed_at, Some(time(0)));
        assert_eq!(state.caught_up_at, Some(time(0)));
    }
}

#[tokio::test(start_paused = true)]
async fn every_round_publishes_its_state() {
    let mut harness = Harness::new(0);
    harness.probe.push_partition_count(Ok(None));
    assert_eq!(harness.round().await, Err(RoundFailed));
    assert_eq!(
        harness.published(),
        TailState {
            fault: Some(TopicFault::Missing),
            ..TailState::default()
        }
    );

    harness.passes_start_up(4, 9);
    assert_eq!(harness.round().await, Ok(()));
    assert_eq!(
        harness.published(),
        TailState {
            fault: None,
            started: true,
            position: Some(Cursor::new(9)),
            earliest: Some(Cursor::new(4)),
            end: Some(Cursor::new(9)),
            broker_reachable: true,
            observations: 1,
            observed_at: Some(time(0)),
            caught_up_at: Some(time(0)),
            rejected: 0,
            retention_losses: 0,
        }
    );

    harness.fails_every_step();
    assert_eq!(harness.round().await, Err(RoundFailed));
    assert!(!harness.published().broker_reachable);

    advance(TailEvents::OBSERVE_EVERY).await;
    harness.fetches(vec![invalid(9)], 10);
    harness.probe.push_watermarks(Ok(watermarks(4, 10)));
    harness.tail.push_seek(Ok(()));
    assert_eq!(harness.round().await, Ok(()));
    let state = harness.published();
    assert_eq!(state.rejected, 1);
    assert_eq!(state.position, Some(Cursor::new(10)));
    assert_eq!(state.observations, 2);
}

#[tokio::test(start_paused = true)]
async fn dropping_the_tail_drops_the_sender_of_its_state() {
    let Harness {
        events, mut state, ..
    } = Harness::new(0);

    drop(events);

    assert!(
        timeout(LIMIT, state.changed())
            .await
            .expect("the receiver notices the dropped sender")
            .is_err()
    );
}
