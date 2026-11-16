use std::collections::VecDeque;
use std::future::poll_fn;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Bytes;
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes};
use event_stream_domain::{Batch, Cursor, TopicError, TopicReplay};
use test_support::fixture::stored_event;
use test_support::log::{LogLine, LogRecorder};
use test_support::topic::ScriptedTopicReplay;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{advance, timeout};
use tracing::Level;

use super::{CLOSE_TIMEOUT, Connection, serve};
use crate::http::session::Session;
use crate::http::test_node::{POSITION, event_json, node, node_with};

/// The ping period and write deadline of these tests; not the default of 15 s, so a lost setting
/// shows.
const KEEP_ALIVE: Duration = Duration::from_secs(7);

/// One millisecond less than [`KEEP_ALIVE`].
const JUST_BEFORE_KEEP_ALIVE: Duration = Duration::from_millis(6_999);

/// One millisecond less than [`CLOSE_TIMEOUT`].
const JUST_BEFORE_CLOSE_TIMEOUT: Duration = Duration::from_millis(999);

const ONE_MILLISECOND: Duration = Duration::from_millis(1);

/// Bounds the awaited end of a pump (time is paused, so nothing really waits).
const WAIT: Duration = Duration::from_millis(1);

/// The yields [`settle`] hands to the scheduler: enough for the pump to run every step that is
/// ready, without moving the clock.
const YIELDS: usize = 64;

/// What the scripted connection does with one write.
#[derive(Debug, Clone, Copy)]
enum Write {
    /// The socket takes the message at once.
    Accept,
    /// The transport fails.
    Fail,
    /// The socket never takes the message.
    Stall,
}

#[derive(Debug, Default)]
struct Script {
    /// Every message the pump tried to write, in order.
    attempts: Mutex<Vec<Message>>,
    /// The behaviour of the next writes; a write after the script is accepted.
    writes: Mutex<VecDeque<Write>>,
}

impl Script {
    fn attempts(&self) -> Vec<Message> {
        self.attempts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn then(&self, write: Write) {
        self.writes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(write);
    }
}

/// The sending end of the inbound messages of a [`ScriptedConnection`].
type InboundSender = mpsc::UnboundedSender<Result<Message, axum::Error>>;

/// A connection whose inbound messages come from a channel (`None` once the sender is dropped)
/// and whose writes follow a script.
#[derive(Debug)]
struct ScriptedConnection {
    inbound: mpsc::UnboundedReceiver<Result<Message, axum::Error>>,
    script: Arc<Script>,
}

impl Connection for ScriptedConnection {
    fn receive(&mut self) -> impl Future<Output = Option<Result<Message, axum::Error>>> + Send {
        self.inbound.recv()
    }

    fn transmit(
        &mut self,
        message: Message,
    ) -> impl Future<Output = Result<(), axum::Error>> + Send {
        self.script
            .attempts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(message);
        let write = self
            .script
            .writes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or(Write::Accept);
        async move {
            match write {
                Write::Accept => Ok(()),
                Write::Fail => Err(axum::Error::new("the peer reset the connection")),
                Write::Stall => std::future::pending().await,
            }
        }
    }
}

/// A replay whose pages are answered from a channel, in order; a page waits until its answer is
/// sent. The receiver is polled under its lock, which no await holds.
#[derive(Debug)]
struct GatedReplay {
    answers: Mutex<mpsc::UnboundedReceiver<Result<Batch, TopicError>>>,
    calls: Mutex<Vec<Cursor>>,
}

impl GatedReplay {
    fn new() -> (Arc<Self>, mpsc::UnboundedSender<Result<Batch, TopicError>>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let replay = Arc::new(Self {
            answers: Mutex::new(receiver),
            calls: Mutex::new(Vec::new()),
        });
        (replay, sender)
    }

    fn calls(&self) -> Vec<Cursor> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    async fn gated_page(&self, from: Cursor) -> Result<Batch, TopicError> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(from);
        let answer = poll_fn(|context| {
            self.answers
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .poll_recv(context)
        })
        .await;
        match answer {
            Some(answer) => answer,
            None => std::future::pending().await,
        }
    }
}

#[async_trait]
impl TopicReplay for GatedReplay {
    async fn page(&self, from: Cursor) -> Result<Batch, TopicError> {
        self.gated_page(from).await
    }
}

/// The WebSocket frame of the event `id` of `event_type`.
fn frame(id: u64, event_type: &str) -> Message {
    Message::Text(Utf8Bytes::from(format!(
        r#"{{"id":"{id}","type":"{event_type}","data":{}}}"#,
        event_json(id)
    )))
}

fn error(reason: &str) -> Message {
    Message::Text(Utf8Bytes::from(format!(r#"{{"error":"{reason}"}}"#)))
}

fn ping() -> Message {
    Message::Ping(Bytes::new())
}

fn close(code: u16) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: Utf8Bytes::from_static(""),
    }))
}

/// A running pump and the ends the test holds.
struct Pump {
    inbound: Option<InboundSender>,
    script: Arc<Script>,
    task: JoinHandle<()>,
}

/// A scripted connection that runs `hook` when the pump asks for its `at`-th inbound read (from
/// 1): in the turn of that read, after the turn asked `replaying()` and before it polls its step.
struct HookedConnection {
    scripted: ScriptedConnection,
    reads: usize,
    at: usize,
    hook: Option<Box<dyn FnOnce() + Send>>,
}

impl Connection for HookedConnection {
    fn receive(&mut self) -> impl Future<Output = Option<Result<Message, axum::Error>>> + Send {
        self.reads = self.reads.saturating_add(1);
        if self.reads == self.at
            && let Some(hook) = self.hook.take()
        {
            hook();
        }
        self.scripted.receive()
    }

    fn transmit(
        &mut self,
        message: Message,
    ) -> impl Future<Output = Result<(), axum::Error>> + Send {
        self.scripted.transmit(message)
    }
}

/// A scripted connection whose first writes follow `writes`, the sender of its inbound messages
/// and its script.
fn scripted(writes: &[Write]) -> (ScriptedConnection, InboundSender, Arc<Script>) {
    let (sender, inbound) = mpsc::unbounded_channel();
    let script = Arc::new(Script::default());
    for write in writes {
        script.then(*write);
    }
    let connection = ScriptedConnection {
        inbound,
        script: Arc::clone(&script),
    };
    (connection, sender, script)
}

async fn spawn(session: Session) -> Pump {
    spawn_with(session, &[]).await
}

/// Spawns the pump of `session` over a scripted connection whose first writes follow `writes`.
async fn spawn_with(session: Session, writes: &[Write]) -> Pump {
    let (connection, sender, script) = scripted(writes);
    launch(connection, session, sender, script).await
}

/// Spawns the pump of `session` over a scripted connection that runs `hook` at its `at`-th
/// inbound read.
async fn spawn_hooked(session: Session, at: usize, hook: impl FnOnce() + Send + 'static) -> Pump {
    let (scripted, sender, script) = scripted(&[]);
    let connection = HookedConnection {
        scripted,
        reads: 0,
        at,
        hook: Some(Box::new(hook)),
    };
    launch(connection, session, sender, script).await
}

/// Spawns the pump of `session` over `connection` and lets it run its first turn, so that its
/// ping timer starts before the test moves the clock.
async fn launch(
    connection: impl Connection + 'static,
    session: Session,
    sender: InboundSender,
    script: Arc<Script>,
) -> Pump {
    let pump = Pump {
        inbound: Some(sender),
        script,
        task: tokio::spawn(serve(connection, session, KEEP_ALIVE)),
    };
    settle().await;
    pump
}

/// Hands the scheduler enough turns for every task to run what is ready; the clock does not move.
async fn settle() {
    for _ in 0..YIELDS {
        tokio::task::yield_now().await;
    }
}

/// Moves the clock by `duration`, then lets every task run.
async fn pass_time(duration: Duration) {
    advance(duration).await;
    settle().await;
}

impl Pump {
    fn send(&self, message: Message) {
        self.inbound.as_ref().unwrap().send(Ok(message)).unwrap();
    }

    fn fail_inbound(&self) {
        self.inbound
            .as_ref()
            .unwrap()
            .send(Err(axum::Error::new("the frame is too long")))
            .unwrap();
    }

    /// Ends the inbound side: `receive` answers `None`.
    fn hang_up(&mut self) {
        self.inbound = None;
    }

    async fn written(&self) -> Vec<Message> {
        settle().await;
        self.script.attempts()
    }

    /// True when the pump returned, after the scheduler ran what was ready.
    async fn ended(&self) -> bool {
        settle().await;
        self.task.is_finished()
    }

    /// Awaits the end of the pump without moving the clock further than [`WAIT`].
    async fn join(self) {
        timeout(WAIT, self.task)
            .await
            .expect("the pump ends")
            .unwrap();
    }
}

fn lines_of(recorder: &LogRecorder, message: &str) -> Vec<LogLine> {
    recorder
        .lines()
        .into_iter()
        .filter(|line| line.message == message)
        .collect()
}

/// The warn entry `message` with `fields` of the module `module` of `http`.
fn warn_line(module: &str, message: &str, fields: &[(&str, &str)]) -> LogLine {
    LogLine {
        level: Level::WARN,
        target: format!("event_stream_application::http::{module}"),
        message: message.to_owned(),
        fields: fields
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    }
}

fn socket_line(message: &str, fields: &[(&str, &str)]) -> LogLine {
    warn_line("socket", message, fields)
}

#[test]
fn the_close_timeout_is_one_second() {
    assert_eq!(CLOSE_TIMEOUT, Duration::from_secs(1));
}

#[tokio::test(start_paused = true)]
async fn each_queued_event_of_a_subscribed_type_is_one_text_frame() {
    let node = node();
    let pump = spawn(node.session(&["user.created"], None)).await;

    node.pass(1_001, "user.created");
    node.pass(1_002, "order.paid");
    node.pass(1_003, "user.created");

    assert_eq!(
        pump.written().await,
        [frame(1_001, "user.created"), frame(1_003, "user.created")]
    );
    assert!(!pump.ended().await);
}

#[tokio::test(start_paused = true)]
async fn an_invalid_command_is_answered_and_a_valid_one_is_not() {
    let node = node();
    let pump = spawn(node.session(&[], None)).await;

    pump.send(Message::Text(Utf8Bytes::from_static("subscribe")));
    pump.send(Message::Binary(Bytes::from_static(b"{}")));
    pump.send(Message::Text(Utf8Bytes::from_static(
        r#"{"action":"subscribe","types":["order.paid"]}"#,
    )));
    let replies = pump.written().await;
    node.pass(1_001, "order.paid");

    assert_eq!(
        replies,
        [
            error("invalid command"),
            error("binary frames are not supported")
        ]
    );
    assert_eq!(
        pump.written().await,
        [
            error("invalid command"),
            error("binary frames are not supported"),
            frame(1_001, "order.paid")
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn after_an_unsubscribe_no_frame_of_the_type_is_written_also_not_a_queued_one() {
    let node = node();
    let pump = spawn(node.session(&["order.paid", "user.created"], None)).await;
    assert_eq!(pump.written().await, Vec::<Message>::new());

    pump.send(Message::Text(Utf8Bytes::from_static(
        r#"{"action":"unsubscribe","types":["order.paid"]}"#,
    )));
    node.pass(1_001, "order.paid");
    node.pass(1_002, "user.created");

    assert_eq!(pump.written().await, [frame(1_002, "user.created")]);
}

#[tokio::test(start_paused = true)]
async fn a_ping_is_sent_every_keep_alive_whether_or_not_frames_were_written() {
    let node = node();
    let pump = spawn(node.session(&["user.created"], None)).await;

    pass_time(JUST_BEFORE_KEEP_ALIVE).await;
    let before = pump.written().await;
    pass_time(ONE_MILLISECOND).await;
    let first = pump.written().await;
    node.pass(1_001, "user.created");
    pass_time(JUST_BEFORE_KEEP_ALIVE).await;
    let with_a_frame = pump.written().await;
    pass_time(ONE_MILLISECOND).await;

    assert_eq!(before, Vec::<Message>::new());
    assert_eq!(first, [ping()]);
    assert_eq!(with_a_frame, [ping(), frame(1_001, "user.created")]);
    assert_eq!(
        pump.written().await,
        [ping(), frame(1_001, "user.created"), ping()]
    );
}

#[tokio::test(start_paused = true)]
async fn a_client_ping_or_pong_is_not_answered_by_the_pump() {
    let node = node();
    let pump = spawn(node.session(&["user.created"], None)).await;

    pump.send(Message::Ping(Bytes::from_static(b"p")));
    pump.send(Message::Pong(Bytes::from_static(b"p")));

    assert_eq!(pump.written().await, Vec::<Message>::new());
    assert!(!pump.ended().await);
}

#[tokio::test(start_paused = true)]
async fn a_write_the_socket_does_not_take_within_the_keep_alive_evicts_the_subscriber() {
    let (recorder, _guard) = LogRecorder::install();
    let node = node();
    let pump = spawn_with(
        node.session(&["order.paid", "user.created"], None),
        &[Write::Stall],
    )
    .await;
    node.pass(1_001, "user.created");
    settle().await;

    pass_time(JUST_BEFORE_KEEP_ALIVE).await;
    let running = !pump.task.is_finished();
    let quiet = lines_of(&recorder, "subscriber evicted");
    pass_time(ONE_MILLISECOND).await;

    assert!(running);
    assert_eq!(quiet, Vec::<LogLine>::new());
    assert!(pump.ended().await);
    assert_eq!(pump.written().await, [frame(1_001, "user.created")]);
    assert_eq!(
        lines_of(&recorder, "subscriber evicted"),
        [socket_line(
            "subscriber evicted",
            &[("transport", "ws"), ("types", "2"), ("last_id", "1001")]
        )]
    );
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_ping_the_socket_does_not_take_evicts_the_subscriber() {
    let (recorder, _guard) = LogRecorder::install();
    let node = node();
    let pump = spawn_with(node.session(&["user.created"], None), &[Write::Stall]).await;

    pass_time(KEEP_ALIVE).await;
    pass_time(JUST_BEFORE_KEEP_ALIVE).await;
    let running = !pump.task.is_finished();
    pass_time(ONE_MILLISECOND).await;

    assert!(running);
    assert!(pump.ended().await);
    assert_eq!(pump.written().await, [ping()]);
    assert_eq!(
        lines_of(&recorder, "subscriber evicted"),
        [socket_line(
            "subscriber evicted",
            &[("transport", "ws"), ("types", "1"), ("last_id", "1000")]
        )]
    );
}

#[tokio::test(start_paused = true)]
async fn a_reply_the_socket_does_not_take_evicts_the_subscriber() {
    let (recorder, _guard) = LogRecorder::install();
    let node = node();
    let pump = spawn_with(node.session(&[], None), &[Write::Stall]).await;

    pump.send(Message::Binary(Bytes::from_static(b"x")));
    settle().await;
    pass_time(KEEP_ALIVE).await;

    assert!(pump.ended().await);
    assert_eq!(
        pump.written().await,
        [error("binary frames are not supported")]
    );
    assert_eq!(lines_of(&recorder, "subscriber evicted").len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_transport_error_on_a_write_ends_the_connection_without_a_close_frame() {
    let (recorder, _guard) = LogRecorder::install();
    let node = node();
    let pump = spawn_with(node.session(&["user.created"], None), &[Write::Fail]).await;

    node.pass(1_001, "user.created");
    node.pass(1_002, "user.created");

    assert!(pump.ended().await);
    assert_eq!(pump.written().await, [frame(1_001, "user.created")]);
    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn an_ended_inbound_side_ends_the_connection_at_once() {
    let (recorder, _guard) = LogRecorder::install();
    let node = node();
    let mut pump = spawn(node.session(&["user.created"], None)).await;

    pump.hang_up();

    assert!(pump.ended().await);
    assert_eq!(pump.written().await, Vec::<Message>::new());
    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn an_inbound_error_ends_the_connection_at_once() {
    let node = node();
    let pump = spawn(node.session(&["user.created"], None)).await;

    pump.fail_inbound();

    assert!(pump.ended().await);
    assert_eq!(pump.written().await, Vec::<Message>::new());
}

#[tokio::test(start_paused = true)]
async fn a_client_close_frame_ends_the_subscription_after_the_client_hangs_up() {
    let node = node();
    let mut pump = spawn(node.session(&["user.created"], None)).await;

    pump.send(Message::Close(None));
    settle().await;
    pump.send(Message::Binary(Bytes::from_static(b"late")));
    node.pass(1_001, "user.created");
    let lingering = !pump.ended().await;
    pump.hang_up();

    assert!(lingering);
    assert!(pump.ended().await);
    assert_eq!(pump.written().await, Vec::<Message>::new());
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn the_lingering_after_a_client_close_frame_ends_after_one_second() {
    let node = node();
    let pump = spawn(node.session(&["user.created"], None)).await;

    pump.send(Message::Close(None));
    settle().await;
    pass_time(JUST_BEFORE_CLOSE_TIMEOUT).await;
    let lingering = !pump.task.is_finished();
    pass_time(ONE_MILLISECOND).await;

    assert!(lingering);
    assert!(pump.ended().await);
    assert_eq!(pump.written().await, Vec::<Message>::new());
}

#[tokio::test(start_paused = true)]
async fn an_inbound_error_while_lingering_ends_the_connection() {
    let node = node();
    let pump = spawn(node.session(&["user.created"], None)).await;

    pump.send(Message::Close(None));
    settle().await;
    pump.fail_inbound();

    assert!(pump.ended().await);
}

#[tokio::test(start_paused = true)]
async fn a_closed_hub_writes_the_queued_frames_then_close_code_1001_and_lingers() {
    let (recorder, _guard) = LogRecorder::install();
    let node = node();
    let mut pump = spawn(node.session(&["user.created"], None)).await;
    node.pass(1_001, "user.created");
    node.pass(1_002, "user.created");

    node.hub.close();

    assert_eq!(
        pump.written().await,
        [
            frame(1_001, "user.created"),
            frame(1_002, "user.created"),
            close(1_001)
        ]
    );
    assert!(!pump.ended().await);
    pump.hang_up();
    pump.join().await;
    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
}

#[tokio::test(start_paused = true)]
async fn the_lingering_after_close_code_1001_ends_after_one_second() {
    let node = node();
    let pump = spawn(node.session(&["user.created"], None)).await;

    node.hub.close();
    settle().await;
    pass_time(JUST_BEFORE_CLOSE_TIMEOUT).await;
    let lingering = !pump.task.is_finished();
    pass_time(ONE_MILLISECOND).await;

    assert!(lingering);
    assert!(pump.ended().await);
    assert_eq!(pump.written().await, [close(1_001)]);
}

#[tokio::test(start_paused = true)]
async fn a_close_frame_that_cannot_be_written_ends_the_connection_without_lingering() {
    let node = node();
    let pump = spawn_with(node.session(&["user.created"], None), &[Write::Fail]).await;

    node.hub.close();

    assert!(pump.ended().await);
    assert_eq!(pump.written().await, [close(1_001)]);
}

#[tokio::test(start_paused = true)]
async fn a_close_frame_the_socket_does_not_take_ends_the_connection_without_a_log_entry() {
    let (recorder, _guard) = LogRecorder::install();
    let node = node();
    let pump = spawn_with(node.session(&["user.created"], None), &[Write::Stall]).await;

    node.hub.close();
    settle().await;
    pass_time(KEEP_ALIVE).await;

    assert!(pump.ended().await);
    assert_eq!(pump.written().await, [close(1_001)]);
    assert_eq!(recorder.lines(), Vec::<LogLine>::new());
}

fn unavailable() -> Result<Batch, TopicError> {
    Err(TopicError::Unavailable("the broker is away".into()))
}

#[tokio::test(start_paused = true)]
async fn a_replay_that_cannot_read_kafka_logs_replay_failed_and_closes_with_1011() {
    let (recorder, _guard) = LogRecorder::install();
    let replay = ScriptedTopicReplay::new();
    replay.push_page(unavailable());
    let node = node_with(Arc::new(replay.clone()));
    let pump = spawn(node.session(&["user.created"], Some(10))).await;

    assert_eq!(pump.written().await, [close(1_011)]);
    assert_eq!(replay.calls(), [Cursor::new(10)]);
    assert_eq!(
        lines_of(&recorder, "replay failed"),
        [warn_line(
            "subscription",
            "replay failed",
            &[
                ("transport", "ws"),
                ("error", "the topic is unavailable: the broker is away")
            ]
        )]
    );
    pass_time(JUST_BEFORE_CLOSE_TIMEOUT).await;
    let lingering = !pump.task.is_finished();
    pass_time(ONE_MILLISECOND).await;
    assert!(lingering);
    assert!(pump.ended().await);
}

#[tokio::test(start_paused = true)]
async fn a_replay_turn_pings_and_reads_no_inbound_frame_until_its_page_returns() {
    let (replay, answers) = GatedReplay::new();
    let node = node_with(Arc::<GatedReplay>::clone(&replay));
    node.pass(1_001, "user.created");
    let pump = spawn(node.session(&["user.created"], Some(10))).await;
    settle().await;

    pump.send(Message::Binary(Bytes::from_static(b"x")));
    pass_time(KEEP_ALIVE).await;
    pass_time(KEEP_ALIVE).await;
    let during = pump.written().await;
    answers
        .send(Ok(Batch {
            records: Vec::new(),
            next: Cursor::new(POSITION),
        }))
        .unwrap();

    assert_eq!(during, [ping(), ping()]);
    assert_eq!(replay.calls(), [Cursor::new(10)]);
    assert_eq!(
        pump.written().await,
        [
            ping(),
            ping(),
            error("binary frames are not supported"),
            frame(1_001, "user.created")
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_ping_the_socket_does_not_take_during_a_replay_turn_evicts_the_subscriber() {
    let (recorder, _guard) = LogRecorder::install();
    let (replay, _answers) = GatedReplay::new();
    let node = node_with(replay);
    let pump = spawn_with(node.session(&["user.created"], Some(10)), &[Write::Stall]).await;
    settle().await;

    pass_time(KEEP_ALIVE).await;
    pass_time(JUST_BEFORE_KEEP_ALIVE).await;
    let running = !pump.task.is_finished();
    pass_time(ONE_MILLISECOND).await;

    assert!(running);
    assert!(pump.ended().await);
    assert_eq!(pump.written().await, [ping()]);
    assert_eq!(
        lines_of(&recorder, "subscriber evicted"),
        [socket_line(
            "subscriber evicted",
            &[("transport", "ws"), ("types", "1"), ("last_id", "10")]
        )]
    );
}

#[tokio::test(start_paused = true)]
async fn a_ping_that_fails_during_a_replay_turn_ends_the_connection() {
    let (replay, _answers) = GatedReplay::new();
    let node = node_with(replay);
    let pump = spawn_with(node.session(&["user.created"], Some(10)), &[Write::Fail]).await;
    settle().await;

    pass_time(KEEP_ALIVE).await;

    assert!(pump.ended().await);
    assert_eq!(pump.written().await, [ping()]);
    assert_eq!(node.hub.subscriber_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_window_evicted_after_the_check_of_a_live_turn_leaves_the_kafka_read_to_a_replay_turn() {
    let (replay, answers) = GatedReplay::new();
    let node = node_with(Arc::<GatedReplay>::clone(&replay));
    for id in 1_001..=1_005 {
        node.pass(id, "user.created");
    }
    let session = node.session(&["user.created"], Some(POSITION));
    let window = Arc::clone(&node.window);
    // The second read is that of the turn after the start step: its check found the floor 1 000
    // at the window floor, then the hook evicts the events 1 001 to 1 005 before its step reads.
    let pump = spawn_hooked(session, 2, move || {
        for id in 1_006..=1_105 {
            window.push(Arc::new(stored_event(id, "user.created", &event_json(id))));
        }
    })
    .await;
    let evicted = node.window.start();

    pump.send(Message::Binary(Bytes::from_static(b"x")));
    let before = pump.written().await;
    let calls = replay.calls();
    answers
        .send(Ok(Batch {
            records: Vec::new(),
            next: Cursor::new(1_005),
        }))
        .unwrap();

    assert_eq!(evicted, Cursor::new(1_005));
    assert_eq!(before, Vec::<Message>::new());
    assert_eq!(calls, [Cursor::new(POSITION)]);
    assert_eq!(
        pump.written().await,
        [error("binary frames are not supported")]
    );
    assert_eq!(replay.calls(), [Cursor::new(POSITION)]);
}
