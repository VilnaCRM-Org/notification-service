use std::collections::BTreeSet;
use std::future::poll_fn;
use std::ops::RangeInclusive;
use std::pin::pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use async_trait::async_trait;
use event_stream_domain::{
    Batch, Cursor, EventType, Offset, StoredEvent, TopicError, TopicRecord, TopicReplay,
};
use rstest::rstest;
use test_support::fixture::stored_event;
use test_support::log::{LogLine, LogRecorder};
use test_support::static_topic::StaticTopic;
use test_support::topic::ScriptedTopicReplay;
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinHandle;
use tokio::task::coop::{consume_budget, has_budget_remaining};
use tokio::time::timeout;
use tracing::Level;

use super::{
    EndReason, ReplaySettings, Step, Subscription, SubscriptionRequest, Subscriptions, TooManyTypes,
};
use crate::http::hub::{Hub, HubError, HubSettings};
use crate::status::SubscriberCount as _;
use crate::tail::{EventPublisher as _, TailState};
use crate::window::Window;

/// Bounds every await of these tests (time is paused, so nothing really waits).
const WAIT: Duration = Duration::from_secs(5);

/// The idle steps a helper allows on top of the frames it expects: the start step, one per page
/// and one per queued frame that is not written. A step that only idles ends the helper instead
/// of looping for ever, so a mutant fails an assertion and never hangs a test.
const IDLE_STEPS: usize = 64;

/// The polls [`within_polls`] allows, each followed by a yield to the scheduler. Under paused time
/// a task that keeps yielding stops the clock, so a `timeout` around a future that only yields
/// could never fire; a bounded count of polls fails an assertion instead of hanging the test.
const POLLS: usize = 64;

const HUB: HubSettings = HubSettings {
    max_subscribers: 100,
    buffer: 1_024,
};

const REPLAY: ReplaySettings = ReplaySettings {
    max_events: 1_000_000,
    max_data_bytes: 65_536,
    permits: 4,
};

/// No permit at all: a step that asks for one waits for ever, so a test that completes with these
/// settings took none.
const NO_PERMIT: ReplaySettings = ReplaySettings {
    permits: 0,
    ..REPLAY
};

/// The type of the event `id` in these tests: odd ids are `a`, even ids `b`.
const fn type_of(id: u64) -> &'static str {
    if id % 2 == 1 { "a" } else { "b" }
}

fn names(names: &[&str]) -> BTreeSet<EventType> {
    names
        .iter()
        .map(|name| EventType::new((*name).to_owned()).unwrap())
        .collect()
}

fn name(name: &str) -> EventType {
    EventType::new(name.to_owned()).unwrap()
}

fn event(id: u64, event_type: &str) -> StoredEvent {
    stored_event(id, event_type, &format!(r#"{{"n":{id}}}"#))
}

/// The record at the offset of the event `id` (offset `id - 1`) with a valid envelope.
fn record(id: u64, event_type: &str) -> TopicRecord {
    let value = format!(
        r#"{{"id":"00000000-0000-0000-0000-{id:012x}","type":"{event_type}","occurredAt":"2026-01-01T00:00:00Z","data":{{"n":{id}}}}}"#
    );
    TopicRecord {
        offset: Offset::new(id.saturating_sub(1)),
        value: Some(value.into_bytes()),
    }
}

/// A record at the offset of the event `id` whose value is not an event.
fn invalid(id: u64, value: Option<&str>) -> TopicRecord {
    TopicRecord {
        offset: Offset::new(id.saturating_sub(1)),
        value: value.map(|text| text.as_bytes().to_vec()),
    }
}

fn records(ids: RangeInclusive<u64>) -> Vec<TopicRecord> {
    ids.map(|id| record(id, type_of(id))).collect()
}

fn page(ids: RangeInclusive<u64>, next: u64) -> Batch {
    Batch {
        records: records(ids),
        next: Cursor::new(next),
    }
}

const fn empty(next: u64) -> Batch {
    Batch {
        records: Vec::new(),
        next: Cursor::new(next),
    }
}

fn unavailable() -> Result<Batch, TopicError> {
    Err(TopicError::Unavailable("the broker is away".into()))
}

const fn below_earliest(earliest: u64) -> Result<Batch, TopicError> {
    Err(TopicError::BelowEarliest {
        earliest: Cursor::new(earliest),
    })
}

fn odd(ids: RangeInclusive<u64>) -> Vec<u64> {
    ids.filter(|id| id % 2 == 1).collect()
}

fn all(ids: RangeInclusive<u64>) -> Vec<u64> {
    ids.collect()
}

/// A started node: the hub, the window, the sender of the tail state and the subscriptions.
struct Node {
    hub: Arc<Hub>,
    window: Arc<Window>,
    tail: watch::Sender<TailState>,
    subscriptions: Subscriptions,
}

/// A node whose hub position and window floor are `floor`, with a window of `window_events`.
fn node_with(
    floor: u64,
    replay: Arc<dyn TopicReplay>,
    settings: ReplaySettings,
    hub: HubSettings,
    window_events: usize,
) -> Node {
    let hub = Hub::new(hub);
    hub.start(Cursor::new(floor));
    let window = Arc::new(Window::new(window_events, usize::MAX));
    window.restart(Cursor::new(floor));
    let (tail, receiver) = watch::channel(TailState::default());
    let subscriptions = Subscriptions::new(
        Arc::clone(&hub),
        Arc::clone(&window),
        replay,
        receiver,
        settings,
    );
    Node {
        hub,
        window,
        tail,
        subscriptions,
    }
}

fn node(floor: u64, replay: Arc<dyn TopicReplay>, settings: ReplaySettings) -> Node {
    node_with(floor, replay, settings, HUB, usize::MAX)
}

/// A node at `floor` whose replay pends on any page: a test on it reads no broker.
fn quiet_node(floor: u64) -> (Node, ScriptedTopicReplay) {
    let replay = ScriptedTopicReplay::new();
    (node(floor, Arc::new(replay.clone()), NO_PERMIT), replay)
}

impl Node {
    /// What the tail does with a valid event: the window first, then the hub.
    fn pass(&self, id: u64, event_type: &str) {
        let event = event(id, event_type);
        self.window.push(Arc::new(event.clone()));
        self.hub.publish(&event);
    }

    fn pass_all(&self, ids: RangeInclusive<u64>) {
        for id in ids {
            self.pass(id, type_of(id));
        }
    }

    fn open(&self, types: &[&str], cursor: Option<u64>) -> Subscription {
        self.subscriptions
            .open(SubscriptionRequest {
                types: names(types),
                cursor: cursor.map(Cursor::new),
            })
            .unwrap()
    }

    /// One more successful watermark read of the tail, with the end `end`.
    fn observe(&self, end: u64) {
        self.tail.send_modify(|state| {
            state.observations = state.observations.saturating_add(1);
            state.end = Some(Cursor::new(end));
        });
    }
}

/// One step within `WAIT`; panics when it pends.
async fn step(subscription: &mut Subscription) -> Step {
    timeout(WAIT, subscription.step())
        .await
        .expect("a step within the wait")
}

async fn idle(subscription: &mut Subscription) {
    assert!(matches!(step(subscription).await, Step::Idle));
}

/// The ids of the next `count` frames, read step by step with at most `IDLE_STEPS` idle steps;
/// fewer when the subscription idles longer, panics when a step pends or the subscription ends.
async fn ids(subscription: &mut Subscription, count: usize) -> Vec<u64> {
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count.saturating_add(IDLE_STEPS) {
        if ids.len() == count {
            break;
        }
        match step(subscription).await {
            Step::Frame(frame) => ids.push(frame.sequence.value()),
            Step::Idle => {}
            Step::End(_) => panic!("the subscription ended"),
        }
    }
    ids
}

/// True when a step pends for `WAIT` before any frame or end.
async fn waits(subscription: &mut Subscription) -> bool {
    for _ in 0..IDLE_STEPS {
        match timeout(WAIT, subscription.step()).await {
            Err(_elapsed) => return true,
            Ok(Step::Idle) => {}
            Ok(Step::Frame(_) | Step::End(_)) => return false,
        }
    }
    false
}

fn replay_capped_lines(recorder: &LogRecorder) -> Vec<LogLine> {
    recorder
        .lines()
        .into_iter()
        .filter(|line| line.message == "replay capped")
        .collect()
}

/// The `(name, value)` pairs of one event, in declaration order.
type Fields = Vec<(String, String)>;

/// Collects the fields of one event, except the message, as the JSON log writes them: a number
/// bare, any other value as a quoted string.
#[derive(Default)]
struct JsonFields(Fields);

impl JsonFields {
    fn push(&mut self, field: &tracing::field::Field, value: String) {
        if field.name() != "message" {
            self.0.push((field.name().to_owned(), value));
        }
    }
}

impl tracing::field::Visit for JsonFields {
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.push(field, value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.push(field, format!("\"{value:?}\""));
    }
}

/// Records the fields of every event of the current thread, as [`JsonFields`] renders them, while
/// its guard lives.
#[derive(Clone, Default)]
struct FieldKinds(Arc<Mutex<Vec<Fields>>>);

impl FieldKinds {
    fn install() -> (Self, tracing::subscriber::DefaultGuard) {
        let kinds = Self::default();
        let guard = tracing::subscriber::set_default(kinds.clone());
        (kinds, guard)
    }

    fn events(&self) -> Vec<Fields> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl tracing::Subscriber for FieldKinds {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut visitor = JsonFields::default();
        event.record(&mut visitor);
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(visitor.0);
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

/// Polls one `step` future with a waker that does nothing, and once more when it pended.
fn poll_twice(subscription: &mut Subscription) -> (Poll<Step>, Option<Poll<Step>>) {
    let mut future = pin!(subscription.step());
    let mut context = Context::from_waker(Waker::noop());
    let first = future.as_mut().poll(&mut context);
    let second = first
        .is_pending()
        .then(|| future.as_mut().poll(&mut context));
    (first, second)
}

/// Polls `future` with the context of the test task at most [`POLLS`] times, with a yield to the
/// scheduler after each poll that pended; `None` when it is still pending after the last one.
async fn within_polls<F: Future>(future: F) -> Option<F::Output> {
    let mut future = pin!(future);
    for _ in 0..POLLS {
        if let Poll::Ready(output) =
            poll_fn(|context| Poll::Ready(future.as_mut().poll(context))).await
        {
            return Some(output);
        }
        tokio::task::yield_now().await;
    }
    None
}

/// Spends the cooperative budget of the test task: until the task yields, every budget point it
/// polls answers `Pending`.
async fn spend_budget() {
    while has_budget_remaining() {
        consume_budget().await;
    }
}

const fn assert_send<T: Send>(_value: &T) {}

/// A replay whose pages wait at a gate: each [`GatedReplay::release`] lets one page return, in
/// the order the pages arrived. A page answers an empty batch one position further.
#[derive(Debug)]
struct GatedReplay {
    calls: Mutex<Vec<Cursor>>,
    gate: Semaphore,
}

impl GatedReplay {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            gate: Semaphore::new(0),
        })
    }

    fn calls(&self) -> Vec<Cursor> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn release(&self) {
        self.gate.add_permits(1);
    }

    async fn gated_page(&self, from: Cursor) -> Result<Batch, TopicError> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(from);
        self.gate.acquire().await.unwrap().forget();
        Ok(Batch {
            records: Vec::new(),
            next: from.advanced_by(1),
        })
    }
}

#[async_trait]
impl TopicReplay for GatedReplay {
    async fn page(&self, from: Cursor) -> Result<Batch, TopicError> {
        self.gated_page(from).await
    }
}

fn cursors(values: &[u64]) -> Vec<Cursor> {
    values.iter().copied().map(Cursor::new).collect()
}

/// Runs one step of `subscription` in a task of its own.
fn spawn_step(mut subscription: Subscription) -> JoinHandle<(Subscription, Step)> {
    tokio::spawn(async move {
        let step = subscription.step().await;
        (subscription, step)
    })
}

/// Lets every spawned task run until it waits (time is paused, so the sleep only yields).
async fn settle() {
    tokio::time::sleep(Duration::from_millis(1)).await;
}

async fn joined(handle: JoinHandle<(Subscription, Step)>) -> (Subscription, Step) {
    timeout(WAIT, handle).await.unwrap().unwrap()
}

// ----- constants and open --------------------------------------------------------------------

#[test]
fn the_constants_hold_the_limits_of_the_protocol() {
    assert_eq!(Subscription::MAX_TYPES, 100);
    assert_eq!(Subscription::REPLAY_PAGE.rows, 500);
    assert_eq!(Subscription::REPLAY_PAGE.bytes, 1_048_576);
    assert_eq!(Subscription::MAX_STALLS, 3);
}

#[test]
fn too_many_types_reads_as_the_limit_of_a_connection() {
    assert_eq!(
        TooManyTypes.to_string(),
        "a connection subscribes to at most 100 event types"
    );
}

#[test]
fn the_subscriptions_hold_a_semaphore_of_the_configured_permits() {
    let replay = ScriptedTopicReplay::new();
    let node = node(0, Arc::new(replay), REPLAY);

    assert_eq!(node.subscriptions.permits.available_permits(), 4);
}

#[test]
fn open_answers_the_refusal_of_the_hub_and_registers_nothing() {
    let (node, _replay) = quiet_node(0);
    let request = SubscriptionRequest {
        types: names(&["a"]),
        cursor: None,
    };
    let not_started = Subscriptions::new(
        Hub::new(HUB),
        Arc::clone(&node.window),
        Arc::new(ScriptedTopicReplay::new()),
        node.tail.subscribe(),
        REPLAY,
    );
    assert_eq!(
        not_started.open(request.clone()).unwrap_err(),
        HubError::NotReady
    );

    let full = node_with(
        0,
        Arc::new(ScriptedTopicReplay::new()),
        REPLAY,
        HubSettings {
            max_subscribers: 1,
            buffer: 1,
        },
        usize::MAX,
    );
    let _held = full.open(&["a"], None);
    assert_eq!(
        full.subscriptions.open(request.clone()).unwrap_err(),
        HubError::AtCapacity
    );

    node.hub.close();
    assert_eq!(
        node.subscriptions.open(request).unwrap_err(),
        HubError::Closed
    );
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[test]
fn open_registers_the_types_with_since_zero_and_sets_the_floor_and_target() {
    let (node, replay) = quiet_node(1_000);

    let live = node.open(&["a", "b"], None);
    let resumed = node.open(&["a"], Some(950));

    assert_eq!(node.hub.subscriber_count(), 2);
    assert_eq!(node.hub.type_count(), 2);
    assert_eq!(live.type_count(), 2);
    assert_eq!(live.types.get(&name("a")), Some(&Cursor::START));
    assert_eq!(live.types.get(&name("b")), Some(&Cursor::START));
    assert_eq!(live.position(), Cursor::new(1_000));
    assert_eq!(live.target, Some(Cursor::new(1_000)));
    assert_eq!(resumed.position(), Cursor::new(950));
    assert_eq!(resumed.target, None);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}

#[tokio::test(start_paused = true)]
async fn the_step_and_next_futures_and_the_subscription_are_send() {
    let (node, _replay) = quiet_node(0);
    let mut subscription = node.open(&["a"], None);

    assert_send(&subscription.step());
    assert_send(&subscription.next());
    assert_send(&subscription);
}

// ----- resume cases ----------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn without_a_cursor_the_subscription_is_live_above_the_hub_position_and_reads_no_broker() {
    let (node, replay) = quiet_node(900);
    node.pass_all(901..=1_000);

    let mut subscription = node.open(&["a"], None);
    node.pass_all(1_001..=1_004);

    assert_eq!(ids(&mut subscription, 2).await, [1_001, 1_003]);
    assert!(waits(&mut subscription).await);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
    assert_eq!(subscription.position(), Cursor::new(1_003));
}

#[tokio::test(start_paused = true)]
async fn a_cursor_inside_the_window_is_served_from_the_window_then_live_without_a_permit() {
    let (node, replay) = quiet_node(900);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a"], Some(950));

    assert_eq!(ids(&mut subscription, 25).await, odd(951..=999));
    node.pass_all(1_001..=1_003);
    assert_eq!(ids(&mut subscription, 2).await, [1_001, 1_003]);
    assert!(waits(&mut subscription).await);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}

#[tokio::test(start_paused = true)]
async fn a_long_window_resume_is_read_in_pages_of_at_most_500_events() {
    let (node, _replay) = quiet_node(0);
    node.pass_all(1..=1_200);
    let mut subscription = node.open(&["a", "b"], Some(0));
    idle(&mut subscription).await;

    idle(&mut subscription).await;
    assert_eq!(subscription.pending.len(), 500);
    assert_eq!(subscription.position(), Cursor::new(500));
    assert_eq!(ids(&mut subscription, 1_200).await, all(1..=1_200));
}

#[tokio::test(start_paused = true)]
async fn a_cursor_below_the_window_reads_kafka_pages_then_window_pages_then_live() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(page(501..=700, 700)));
    replay.push_page(Ok(page(701..=900, 900)));
    let node = node(900, Arc::new(replay.clone()), REPLAY);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a"], Some(500));

    assert_eq!(ids(&mut subscription, 250).await, odd(501..=999));
    node.pass_all(1_001..=1_003);
    assert_eq!(ids(&mut subscription, 2).await, [1_001, 1_003]);
    assert!(waits(&mut subscription).await);
    assert_eq!(replay.calls(), cursors(&[500, 700]));
}

#[tokio::test(start_paused = true)]
async fn a_cursor_below_the_log_start_replays_from_the_earliest_retained_event() {
    let topic = StaticTopic::new(records(101..=1_000), Cursor::new(100), Cursor::new(1_000));
    let node = node(900, Arc::new(topic), REPLAY);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a", "b"], Some(50));

    idle(&mut subscription).await;
    assert_eq!(subscription.floor, Cursor::new(50));
    idle(&mut subscription).await;
    assert_eq!(subscription.floor, Cursor::new(100));
    assert_eq!(ids(&mut subscription, 900).await, all(101..=1_000));
    assert!(waits(&mut subscription).await);
}

#[tokio::test(start_paused = true)]
async fn a_log_start_above_the_window_floor_continues_from_the_window_floor() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(below_earliest(100));
    let node = node(99, Arc::new(replay.clone()), REPLAY);
    node.pass_all(100..=1_000);
    let mut subscription = node.open(&["a", "b"], Some(50));

    idle(&mut subscription).await;
    idle(&mut subscription).await;
    assert_eq!(subscription.floor, Cursor::new(99));
    assert_eq!(ids(&mut subscription, 901).await, all(100..=1_000));
    assert_eq!(replay.calls(), cursors(&[50]));
}

#[rstest]
#[case::far_below(200, 100, 800, true)]
#[case::one_below(200, 799, 800, true)]
#[case::at_the_cap(200, 800, 800, false)]
#[case::max_one_below(1, 998, 999, true)]
#[case::max_one_at(1, 999, 999, false)]
#[case::max_large(u64::MAX, 0, 0, false)]
#[tokio::test(start_paused = true)]
async fn the_cap_raises_a_floor_more_than_max_events_below_the_hub_position(
    #[case] max_events: u64,
    #[case] cursor: u64,
    #[case] floor: u64,
    #[case] capped: bool,
) {
    let (recorder, _guard) = LogRecorder::install();
    let replay = ScriptedTopicReplay::new();
    let settings = ReplaySettings {
        max_events,
        ..REPLAY
    };
    let node = node(1_000, Arc::new(replay.clone()), settings);
    let mut subscription = node.open(&["a"], Some(cursor));

    idle(&mut subscription).await;

    assert_eq!(subscription.floor, Cursor::new(floor));
    assert_eq!(subscription.target, Some(Cursor::new(1_000)));
    assert_eq!(node.hub.replay_capped(), u64::from(capped));
    let lines = replay_capped_lines(&recorder);
    if capped {
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].level, Level::INFO);
        assert_eq!(lines[0].field("from"), Some(cursor.to_string().as_str()));
        assert_eq!(lines[0].field("to"), Some(floor.to_string().as_str()));
    } else {
        assert_eq!(lines, Vec::<LogLine>::new());
    }
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}

#[tokio::test(start_paused = true)]
async fn replay_capped_writes_from_and_to_as_numbers_like_the_entries_of_the_tail() {
    let (kinds, _guard) = FieldKinds::install();
    let settings = ReplaySettings {
        max_events: 200,
        ..NO_PERMIT
    };
    let node = node(1_000, Arc::new(ScriptedTopicReplay::new()), settings);
    let mut subscription = node.open(&["a"], Some(100));

    idle(&mut subscription).await;

    assert_eq!(
        kinds.events(),
        [vec![
            ("from".to_owned(), "100".to_owned()),
            ("to".to_owned(), "800".to_owned()),
        ]]
    );
}

#[tokio::test(start_paused = true)]
async fn a_capped_replay_delivers_only_the_events_above_the_raised_floor() {
    let (recorder, _guard) = LogRecorder::install();
    let topic = StaticTopic::new(records(1..=1_000), Cursor::START, Cursor::new(1_000));
    let settings = ReplaySettings {
        max_events: 200,
        ..REPLAY
    };
    let node = node(900, Arc::new(topic), settings);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a", "b"], Some(100));

    assert_eq!(ids(&mut subscription, 200).await, all(801..=1_000));
    node.pass_all(1_001..=1_001);
    assert_eq!(ids(&mut subscription, 1).await, [1_001]);
    assert_eq!(node.hub.replay_capped(), 1);
    assert_eq!(replay_capped_lines(&recorder).len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_subscription_without_types_and_a_cursor_starts_live_at_the_hub_position() {
    let (recorder, _guard) = LogRecorder::install();
    let replay = ScriptedTopicReplay::new();
    let settings = ReplaySettings {
        max_events: 200,
        ..NO_PERMIT
    };
    let node = node(900, Arc::new(replay.clone()), settings);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&[], Some(500));

    idle(&mut subscription).await;

    assert_eq!(subscription.floor, Cursor::new(1_000));
    assert_eq!(subscription.target, Some(Cursor::new(1_000)));
    assert_eq!(node.hub.replay_capped(), 0);
    assert_eq!(replay_capped_lines(&recorder), Vec::<LogLine>::new());
    assert!(waits(&mut subscription).await);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}

#[tokio::test(start_paused = true)]
async fn a_cursor_at_the_hub_position_reads_nothing_and_is_live() {
    let (node, replay) = quiet_node(900);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a", "b"], Some(1_000));

    idle(&mut subscription).await;
    assert_eq!(subscription.floor, Cursor::new(1_000));
    assert_eq!(subscription.target, Some(Cursor::new(1_000)));
    node.pass_all(1_001..=1_002);
    assert_eq!(ids(&mut subscription, 2).await, [1_001, 1_002]);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}

#[tokio::test(start_paused = true)]
async fn a_cursor_ahead_of_the_tail_waits_for_two_observations_then_is_live_above_it() {
    let (node, replay) = quiet_node(1_000);
    node.tail.send_modify(|state| state.observations = 7);
    let mut subscription = node.open(&["a", "b"], Some(1_005));
    node.pass_all(1_001..=1_010);

    assert!(waits(&mut subscription).await);
    assert_eq!(subscription.ahead_since, Some(7));
    node.observe(1_010);
    assert!(waits(&mut subscription).await);
    node.observe(1_010);
    assert_eq!(ids(&mut subscription, 5).await, all(1_006..=1_010));
    assert_eq!(subscription.ahead_since, None);
    node.pass_all(1_011..=1_011);
    assert_eq!(ids(&mut subscription, 1).await, [1_011]);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}

#[tokio::test(start_paused = true)]
async fn a_cursor_above_the_topic_end_is_clamped_to_the_end_of_the_second_observation() {
    let (node, _replay) = quiet_node(1_000);
    let mut subscription = node.open(&["a", "b"], Some(5_000));
    node.pass_all(1_001..=1_010);

    assert!(waits(&mut subscription).await);
    node.observe(1_008);
    node.observe(1_010);
    node.pass_all(1_011..=1_012);

    assert_eq!(ids(&mut subscription, 2).await, [1_011, 1_012]);
    assert_eq!(subscription.position(), Cursor::new(1_012));
}

#[tokio::test(start_paused = true)]
async fn a_dropped_tail_sender_ends_the_wait_of_a_cursor_ahead() {
    let (node, _replay) = quiet_node(1_000);
    let mut subscription = node.open(&["a"], Some(1_005));
    assert!(waits(&mut subscription).await);

    drop(node.tail);

    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::HubClosed)
    ));
}

#[tokio::test(start_paused = true)]
async fn a_step_future_dropped_during_the_wait_keeps_the_count_it_started_from() {
    let (node, _replay) = quiet_node(1_000);
    node.tail.send_modify(|state| state.observations = 3);
    let mut subscription = node.open(&["a"], Some(1_005));

    assert!(timeout(WAIT, subscription.step()).await.is_err());
    assert_eq!(subscription.ahead_since, Some(3));
    node.observe(1_010);
    assert!(timeout(WAIT, subscription.step()).await.is_err());
    assert_eq!(subscription.ahead_since, Some(3));
    node.observe(1_010);

    idle(&mut subscription).await;
    assert_eq!(subscription.ahead_since, None);
    assert_eq!(subscription.floor, Cursor::new(1_005));
    assert_eq!(subscription.target, Some(Cursor::new(1_000)));
}

#[tokio::test(start_paused = true)]
async fn a_jump_of_more_than_two_observations_also_ends_the_wait_of_a_cursor_ahead() {
    let (node, _replay) = quiet_node(1_000);
    node.tail.send_modify(|state| state.observations = 7);
    let mut subscription = node.open(&["a"], Some(1_005));
    assert!(waits(&mut subscription).await);

    node.tail.send_modify(|state| {
        state.observations = 10;
        state.end = Some(Cursor::new(1_010));
    });

    idle(&mut subscription).await;
    assert_eq!(subscription.floor, Cursor::new(1_005));
    assert_eq!(subscription.ahead_since, None);
}

#[tokio::test(start_paused = true)]
async fn an_event_passed_while_nobody_subscribed_to_its_type_is_served_once_from_the_window() {
    let (node, _replay) = quiet_node(1_000);
    node.pass(1_001, "a");
    let mut subscription = node.open(&["a"], Some(1_000));

    assert_eq!(ids(&mut subscription, 1).await, [1_001]);
    node.pass(1_002, "a");
    assert_eq!(ids(&mut subscription, 1).await, [1_002]);
    assert!(waits(&mut subscription).await);
}

// ----- permits ---------------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn with_every_permit_held_by_a_parked_page_read_the_next_replay_reads_nothing_until_one_returns()
 {
    let replay = GatedReplay::new();
    let node = node(900, Arc::<GatedReplay>::clone(&replay), REPLAY);
    let mut handles = Vec::new();
    for cursor in 100..=104 {
        let mut subscription = node.open(&["a"], Some(cursor));
        idle(&mut subscription).await;
        handles.push(spawn_step(subscription));
    }

    settle().await;
    assert_eq!(replay.calls(), cursors(&[100, 101, 102, 103]));
    assert_eq!(node.subscriptions.permits.available_permits(), 0);

    replay.release();
    settle().await;
    assert_eq!(replay.calls(), cursors(&[100, 101, 102, 103, 104]));
    let (first, step) = joined(handles.remove(0)).await;
    assert!(matches!(step, Step::Idle));
    assert_eq!(first.floor, Cursor::new(101));
    for _ in 0..4 {
        replay.release();
    }
    for handle in handles {
        assert!(matches!(joined(handle).await.1, Step::Idle));
    }
    assert_eq!(node.subscriptions.permits.available_permits(), 4);
}

#[tokio::test(start_paused = true)]
async fn permits_are_granted_in_arrival_order() {
    let replay = GatedReplay::new();
    let settings = ReplaySettings {
        permits: 1,
        ..REPLAY
    };
    let node = node(900, Arc::<GatedReplay>::clone(&replay), settings);
    let mut handles = Vec::new();
    for cursor in [300, 100, 200] {
        let mut subscription = node.open(&["a"], Some(cursor));
        idle(&mut subscription).await;
        handles.push(spawn_step(subscription));
        settle().await;
    }

    assert_eq!(replay.calls(), cursors(&[300]));
    replay.release();
    settle().await;
    assert_eq!(replay.calls(), cursors(&[300, 100]));
    replay.release();
    settle().await;
    assert_eq!(replay.calls(), cursors(&[300, 100, 200]));
    replay.release();
    for handle in handles {
        assert!(matches!(joined(handle).await.1, Step::Idle));
    }
}

#[tokio::test(start_paused = true)]
async fn the_wait_for_a_permit_counts_no_stall_and_reads_nothing() {
    let replay = ScriptedTopicReplay::new();
    let settings = ReplaySettings {
        permits: 1,
        ..REPLAY
    };
    let node = node(900, Arc::new(replay.clone()), settings);
    let held = timeout(
        WAIT,
        Arc::clone(&node.subscriptions.permits).acquire_owned(),
    )
    .await
    .unwrap()
    .unwrap();
    let mut subscription = node.open(&["a"], Some(100));
    idle(&mut subscription).await;

    for _ in 0..3 {
        assert!(timeout(WAIT, subscription.step()).await.is_err());
    }
    assert_eq!(subscription.stalls, 0);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());

    drop(held);
    for _ in 0..3 {
        replay.push_page(Ok(empty(100)));
    }
    idle(&mut subscription).await;
    idle(&mut subscription).await;
    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::TopicFailed)
    ));
}

// ----- Kafka pages -----------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_kafka_page_keeps_only_parsing_wanted_records_up_to_the_target_without_a_log_entry() {
    let (recorder, _guard) = LogRecorder::install();
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(Batch {
        records: vec![
            record(1, "a"),
            invalid(2, Some("not json")),
            invalid(3, None),
            record(4, "b"),
            record(5, "a"),
            record(6, "a"),
            record(7, "a"),
        ],
        next: Cursor::new(7),
    }));
    let node = node(5, Arc::new(replay.clone()), REPLAY);
    let mut subscription = node.open(&["a"], Some(0));

    assert_eq!(ids(&mut subscription, 2).await, [1, 5]);
    assert!(subscription.pending.is_empty());
    assert_eq!(subscription.floor, Cursor::new(5));
    node.pass(6, "a");
    assert_eq!(ids(&mut subscription, 1).await, [6]);
    assert!(waits(&mut subscription).await);
    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
    assert_eq!(replay.calls(), cursors(&[0]));
}

#[rstest]
#[case::exactly_500(500, 510, 510)]
#[case::one_more(501, 510, 500)]
#[case::many_more(600, 600, 500)]
#[tokio::test(start_paused = true)]
async fn a_kafka_page_takes_at_most_500_records(
    #[case] count: u64,
    #[case] next: u64,
    #[case] floor: u64,
) {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(Batch {
        records: (1..=count).map(|id| record(id, "a")).collect(),
        next: Cursor::new(next),
    }));
    let node = node(2_000, Arc::new(replay), REPLAY);
    let mut subscription = node.open(&["a"], Some(0));
    idle(&mut subscription).await;

    idle(&mut subscription).await;

    assert_eq!(subscription.pending.len(), 500);
    assert_eq!(subscription.floor, Cursor::new(floor));
    assert_eq!(ids(&mut subscription, 500).await, all(1..=500));
}

#[tokio::test(start_paused = true)]
async fn records_at_or_below_the_floor_and_a_next_below_it_move_nothing_back() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(Batch {
        records: vec![record(3, "a")],
        next: Cursor::new(2),
    }));
    let node = node(10, Arc::new(replay), REPLAY);
    let mut subscription = node.open(&["a"], Some(3));
    idle(&mut subscription).await;

    idle(&mut subscription).await;

    assert_eq!(subscription.floor, Cursor::new(3));
    assert!(subscription.pending.is_empty());
    assert_eq!(subscription.stalls, 1);
}

#[tokio::test(start_paused = true)]
async fn three_kafka_pages_in_a_row_without_a_record_or_progress_end_the_step() {
    let replay = ScriptedTopicReplay::new();
    for answer in [
        Ok(empty(100)),
        Ok(empty(100)),
        Ok(empty(101)),
        Ok(empty(101)),
        Ok(empty(101)),
        Ok(empty(101)),
    ] {
        replay.push_page(answer);
    }
    let node = node(900, Arc::new(replay.clone()), REPLAY);
    let mut subscription = node.open(&["a"], Some(100));
    idle(&mut subscription).await;

    idle(&mut subscription).await;
    idle(&mut subscription).await;
    assert_eq!(subscription.stalls, 2);
    idle(&mut subscription).await;
    assert_eq!(subscription.stalls, 0);
    assert_eq!(subscription.floor, Cursor::new(101));
    idle(&mut subscription).await;
    idle(&mut subscription).await;
    assert_eq!(subscription.stalls, 2);
    assert_eq!(subscription.failure(), None);
    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::TopicFailed)
    ));
    assert_eq!(replay.calls(), cursors(&[100, 100, 100, 101, 101, 101]));
    assert_eq!(
        subscription.failure(),
        Some("three page reads in a row without a record and without progress")
    );
}

#[tokio::test(start_paused = true)]
async fn pages_with_progress_but_no_wanted_record_never_count_as_stalls() {
    let replay = ScriptedTopicReplay::new();
    for id in [102, 104, 106, 108] {
        replay.push_page(Ok(Batch {
            records: vec![record(id, "b")],
            next: Cursor::new(id),
        }));
    }
    let node = node(900, Arc::new(replay), REPLAY);
    let mut subscription = node.open(&["a"], Some(100));
    idle(&mut subscription).await;

    for floor in [102, 104, 106, 108] {
        idle(&mut subscription).await;
        assert_eq!(subscription.stalls, 0);
        assert_eq!(subscription.floor, Cursor::new(floor));
    }
}

#[tokio::test(start_paused = true)]
async fn a_below_earliest_answer_moves_the_floor_and_resets_the_stall_count() {
    let replay = ScriptedTopicReplay::new();
    for answer in [
        Ok(empty(100)),
        Ok(empty(100)),
        below_earliest(150),
        Ok(empty(150)),
        Ok(empty(150)),
        Ok(empty(150)),
    ] {
        replay.push_page(answer);
    }
    let node = node(900, Arc::new(replay), REPLAY);
    let mut subscription = node.open(&["a"], Some(100));
    idle(&mut subscription).await;

    idle(&mut subscription).await;
    idle(&mut subscription).await;
    idle(&mut subscription).await;
    assert_eq!(subscription.floor, Cursor::new(150));
    assert_eq!(subscription.stalls, 0);
    idle(&mut subscription).await;
    idle(&mut subscription).await;
    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::TopicFailed)
    ));
}

#[tokio::test(start_paused = true)]
async fn an_unavailable_page_ends_the_step_at_once() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(unavailable());
    let node = node(900, Arc::new(replay), REPLAY);
    let mut subscription = node.open(&["a"], Some(100));
    idle(&mut subscription).await;
    assert_eq!(subscription.failure(), None);

    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::TopicFailed)
    ));
    assert_eq!(node.subscriptions.permits.available_permits(), 4);
    assert_eq!(
        subscription.failure(),
        Some("the topic is unavailable: the broker is away")
    );
}

// ----- closed hub ------------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_closed_hub_ends_the_start_step() {
    let (node, _replay) = quiet_node(1_000);
    let mut subscription = node.open(&["a"], Some(950));

    node.hub.close();

    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::HubClosed)
    ));
}

#[tokio::test(start_paused = true)]
async fn a_closed_hub_ends_the_step_before_a_window_read() {
    let (node, _replay) = quiet_node(900);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a"], Some(950));
    idle(&mut subscription).await;

    node.hub.close();

    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::HubClosed)
    ));
    assert!(subscription.pending.is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_closed_hub_ends_the_step_before_a_kafka_read() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(empty(200)));
    let node = node(900, Arc::new(replay.clone()), REPLAY);
    let mut subscription = node.open(&["a"], Some(100));
    idle(&mut subscription).await;

    node.hub.close();

    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::HubClosed)
    ));
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
    assert_eq!(subscription.failure(), None);
}

#[tokio::test(start_paused = true)]
async fn a_hub_closed_during_the_wait_for_a_permit_ends_the_step_without_a_kafka_read() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(empty(200)));
    let settings = ReplaySettings {
        permits: 1,
        ..REPLAY
    };
    let node = node(900, Arc::new(replay.clone()), settings);
    let held = timeout(
        WAIT,
        Arc::clone(&node.subscriptions.permits).acquire_owned(),
    )
    .await
    .unwrap()
    .unwrap();
    let mut subscription = node.open(&["a"], Some(100));
    idle(&mut subscription).await;
    let handle = spawn_step(subscription);
    settle().await;

    node.hub.close();
    drop(held);

    assert!(matches!(
        joined(handle).await.1,
        Step::End(EndReason::HubClosed)
    ));
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}

#[tokio::test(start_paused = true)]
async fn a_closed_semaphore_ends_the_step_without_a_kafka_read() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(empty(200)));
    let node = node(900, Arc::new(replay.clone()), REPLAY);
    let mut subscription = node.open(&["a"], Some(100));
    idle(&mut subscription).await;

    node.subscriptions.permits.close();

    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::HubClosed)
    ));
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}

#[tokio::test(start_paused = true)]
async fn a_closed_hub_ends_the_live_step() {
    let (node, _replay) = quiet_node(1_000);
    let mut subscription = node.open(&["a"], None);

    node.hub.close();

    assert!(matches!(
        step(&mut subscription).await,
        Step::End(EndReason::HubClosed)
    ));
}

// ----- falling behind --------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_subscriber_that_falls_behind_catches_up_from_its_queue_then_the_window_then_live() {
    let replay = ScriptedTopicReplay::new();
    let hub = HubSettings {
        max_subscribers: 100,
        buffer: 4,
    };
    let node = node_with(0, Arc::new(replay.clone()), NO_PERMIT, hub, usize::MAX);
    let mut subscription = node.open(&["a"], None);

    node.pass_all(1..=40);

    assert_eq!(ids(&mut subscription, 20).await, odd(1..=39));
    assert_eq!(node.hub.subscriber_count(), 1);
    node.pass_all(41..=42);
    assert_eq!(ids(&mut subscription, 1).await, [41]);
    assert!(waits(&mut subscription).await);
    assert_eq!(node.hub.subscriber_count(), 1);
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}

#[tokio::test(start_paused = true)]
async fn a_subscriber_that_falls_behind_past_the_window_catches_up_from_kafka() {
    let topic = StaticTopic::new(records(1..=40), Cursor::START, Cursor::new(40));
    let hub = HubSettings {
        max_subscribers: 100,
        buffer: 4,
    };
    let node = node_with(0, Arc::new(topic), REPLAY, hub, 5);
    let mut subscription = node.open(&["a"], None);

    node.pass_all(1..=40);
    assert_eq!(node.window.start(), Cursor::new(35));

    assert_eq!(ids(&mut subscription, 20).await, odd(1..=39));
    node.pass_all(41..=41);
    assert_eq!(ids(&mut subscription, 1).await, [41]);
    assert_eq!(node.hub.subscriber_count(), 1);
}

// ----- commands --------------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_type_added_during_a_replay_is_written_only_above_the_hub_position_at_the_command() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(page(501..=700, 700)));
    replay.push_page(Ok(page(701..=900, 900)));
    let node = node(900, Arc::new(replay), REPLAY);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a"], Some(500));

    assert_eq!(ids(&mut subscription, 3).await, [501, 503, 505]);
    subscription.subscribe(names(&["b"])).unwrap();
    assert_eq!(
        subscription.types.get(&name("b")),
        Some(&Cursor::new(1_000))
    );

    assert_eq!(ids(&mut subscription, 247).await, odd(507..=999));
    node.pass_all(1_001..=1_002);
    assert_eq!(ids(&mut subscription, 2).await, [1_001, 1_002]);
    assert!(waits(&mut subscription).await);
}

#[tokio::test(start_paused = true)]
async fn a_type_added_while_behind_is_not_replayed_below_the_hub_position_at_the_command() {
    let hub = HubSettings {
        max_subscribers: 100,
        buffer: 4,
    };
    let node = node_with(
        0,
        Arc::new(ScriptedTopicReplay::new()),
        NO_PERMIT,
        hub,
        usize::MAX,
    );
    let mut subscription = node.open(&["a"], None);
    node.pass_all(1..=12);

    subscription.subscribe(names(&["b"])).unwrap();
    node.pass_all(13..=16);

    assert_eq!(
        ids(&mut subscription, 10).await,
        [1, 3, 5, 7, 9, 11, 13, 14, 15, 16]
    );
    assert!(waits(&mut subscription).await);
}

#[tokio::test(start_paused = true)]
async fn an_unsubscribed_type_is_not_written_from_the_page_the_window_kafka_or_the_queue() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(page(501..=700, 700)));
    replay.push_page(Ok(page(701..=900, 900)));
    let node = node(900, Arc::new(replay), REPLAY);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a", "b"], Some(500));

    assert_eq!(ids(&mut subscription, 2).await, [501, 502]);
    node.pass_all(1_001..=1_002);
    subscription.unsubscribe(&names(&["b"]));
    node.pass_all(1_003..=1_004);

    let mut expected = odd(503..=999);
    expected.extend([1_001, 1_003]);
    assert_eq!(ids(&mut subscription, expected.len()).await, expected);
    assert!(waits(&mut subscription).await);
    assert_eq!(subscription.type_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_subscribe_that_would_pass_100_types_is_refused_and_changes_nothing() {
    let (node, _replay) = quiet_node(0);
    let held: Vec<String> = (0..99).map(|index| format!("t{index}")).collect();
    let held: Vec<&str> = held.iter().map(String::as_str).collect();
    let mut subscription = node.open(&held, None);

    assert_eq!(
        subscription.subscribe(names(&["n1", "n2"])),
        Err(TooManyTypes)
    );
    assert_eq!(subscription.type_count(), 99);
    assert_eq!(node.hub.type_count(), 99);
    assert!(!subscription.types.contains_key(&name("n1")));

    assert_eq!(subscription.subscribe(names(&["n1"])), Ok(()));
    assert_eq!(subscription.type_count(), 100);
    assert_eq!(node.hub.type_count(), 100);
    assert_eq!(subscription.subscribe(names(&["t0", "n1"])), Ok(()));
    assert_eq!(subscription.type_count(), 100);
}

#[tokio::test(start_paused = true)]
async fn subscribing_to_a_held_type_and_unsubscribing_from_an_absent_type_change_nothing() {
    let (node, _replay) = quiet_node(1_000);
    let mut subscription = node.open(&["a"], None);
    node.pass_all(1_001..=1_005);

    subscription.subscribe(names(&["a", "b"])).unwrap();
    subscription.unsubscribe(&names(&["z"]));

    assert_eq!(subscription.types.get(&name("a")), Some(&Cursor::START));
    assert_eq!(
        subscription.types.get(&name("b")),
        Some(&Cursor::new(1_005))
    );
    assert_eq!(subscription.type_count(), 2);
    assert_eq!(node.hub.type_count(), 2);
    assert_eq!(ids(&mut subscription, 3).await, [1_001, 1_003, 1_005]);
}

#[tokio::test(start_paused = true)]
async fn a_subscription_without_types_stays_open_and_receives_again_after_a_subscribe() {
    let (node, _replay) = quiet_node(1_000);
    let mut subscription = node.open(&["a"], None);

    subscription.unsubscribe(&names(&["a"]));
    assert_eq!(subscription.type_count(), 0);
    assert_eq!(node.hub.type_count(), 0);
    node.pass(1_001, "a");
    assert!(waits(&mut subscription).await);

    subscription.subscribe(names(&["a"])).unwrap();
    node.pass(1_002, "a");
    assert_eq!(ids(&mut subscription, 1).await, [1_002]);
}

#[tokio::test(start_paused = true)]
async fn a_type_subscribed_again_does_not_write_its_queued_frame_at_the_new_since() {
    let (node, _replay) = quiet_node(1_000);
    let mut subscription = node.open(&["a"], None);
    node.pass(1_001, "a");

    subscription.unsubscribe(&names(&["a"]));
    subscription.subscribe(names(&["a"])).unwrap();
    assert_eq!(
        subscription.types.get(&name("a")),
        Some(&Cursor::new(1_001))
    );
    node.pass(1_002, "a");

    assert_eq!(ids(&mut subscription, 1).await, [1_002]);
    assert_eq!(subscription.position(), Cursor::new(1_002));
    assert!(waits(&mut subscription).await);
}

// ----- the step loop ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn next_skips_the_idle_steps_returns_the_first_frame_and_none_at_the_end() {
    let (node, _replay) = quiet_node(900);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a"], Some(950));

    let frame = within_polls(subscription.next()).await.unwrap().unwrap();
    assert_eq!(frame.sequence.value(), 951);

    let mut ended = node.open(&["a"], Some(950));
    node.hub.close();
    assert!(within_polls(ended.next()).await.unwrap().is_none());
}

#[tokio::test(start_paused = true)]
async fn next_hands_the_thread_back_at_an_idle_step_once_the_task_budget_is_spent() {
    let (node, _replay) = quiet_node(900);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a"], Some(950));
    let mut next = pin!(subscription.next());

    spend_budget().await;
    let spent = next.as_mut().poll(&mut Context::from_waker(Waker::noop()));
    assert!(
        spent.is_pending(),
        "a spent budget pends at the first idle step: {spent:?}"
    );
    tokio::task::yield_now().await;
    let fresh = poll_fn(|context| Poll::Ready(next.as_mut().poll(context))).await;

    let Poll::Ready(Some(frame)) = fresh else {
        panic!("a fresh budget gives the first frame in one poll: {fresh:?}");
    };
    assert_eq!(frame.sequence.value(), 951);
}

#[tokio::test(start_paused = true)]
async fn a_window_page_without_an_event_yields_before_it_returns_idle() {
    let (node, _replay) = quiet_node(900);
    for id in 901..=1_000 {
        node.pass(id, "b");
    }
    let mut subscription = node.open(&["a"], Some(950));
    idle(&mut subscription).await;

    let (first, second) = poll_twice(&mut subscription);

    assert!(first.is_pending());
    assert!(matches!(second, Some(Poll::Ready(Step::Idle))));
    assert_eq!(subscription.floor, Cursor::new(1_000));
    assert!(subscription.pending.is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_window_page_with_events_returns_idle_at_once() {
    let (node, _replay) = quiet_node(900);
    node.pass_all(901..=1_000);
    let mut subscription = node.open(&["a"], Some(950));
    idle(&mut subscription).await;

    let (first, second) = poll_twice(&mut subscription);

    assert!(matches!(first, Poll::Ready(Step::Idle)));
    assert!(second.is_none());
    assert_eq!(subscription.pending.len(), 25);
    assert_eq!(subscription.floor, Cursor::new(1_000));
}

#[tokio::test(start_paused = true)]
async fn pending_is_replaced_by_a_new_empty_deque_after_it_drained() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(page(101..=120, 120)));
    replay.push_page(Ok(page(121..=140, 140)));
    let node = node(900, Arc::new(replay), REPLAY);
    let mut subscription = node.open(&["a", "b"], Some(100));
    idle(&mut subscription).await;

    idle(&mut subscription).await;
    assert!(subscription.pending.capacity() >= 20);
    assert_eq!(ids(&mut subscription, 20).await, all(101..=120));
    assert_eq!(subscription.pending.capacity(), 0);

    idle(&mut subscription).await;
    assert_eq!(ids(&mut subscription, 9).await, all(121..=129));
    subscription.unsubscribe(&names(&["a", "b"]));
    idle(&mut subscription).await;
    assert!(subscription.pending.is_empty());
    assert_eq!(subscription.pending.capacity(), 0);
}

#[tokio::test(start_paused = true)]
async fn replaying_is_true_only_while_the_next_step_reads_kafka() {
    let replay = ScriptedTopicReplay::new();
    replay.push_page(Ok(page(101..=300, 300)));
    let node = node(900, Arc::new(replay), REPLAY);
    node.pass_all(901..=1_000);

    let mut below = node.open(&["a"], Some(100));
    assert!(!below.replaying());
    idle(&mut below).await;
    assert!(below.replaying());
    idle(&mut below).await;
    assert!(!below.pending.is_empty());
    assert!(!below.replaying());

    let mut at_the_floor = node.open(&["a"], Some(900));
    idle(&mut at_the_floor).await;
    assert!(!at_the_floor.replaying());

    let live = node.open(&["a"], None);
    assert!(!live.replaying());
}

#[tokio::test(start_paused = true)]
async fn replaying_is_false_at_the_target_also_below_the_window_floor() {
    let (node, _replay) = quiet_node(1_000);
    node.window.restart(Cursor::new(2_000));
    let mut subscription = node.open(&["a"], Some(1_000));

    idle(&mut subscription).await;

    assert!(!subscription.replaying());
}
