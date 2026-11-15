mod node;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use event_stream_application::Hub;
use event_stream_domain::Cursor;
use futures_util::StreamExt as _;
use shared_domain::BoxError;
use test_support::log::LogRecorder;
use test_support::static_topic::StaticTopic;
use test_support::topic::{PanickingTopicProbe, ScriptedTopicReplay};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{self, Message};

use self::node::{
    Client, Sse, TYPE, bounded, config, connect, dial, get, listen, parse, text, wait_for_status,
    wait_until_ready, ws_prefix,
};
use super::{Shutdown, bounded_listener, run_until, serve_on, serving_runtime};
use crate::background::STREAM_DRAIN_TIMEOUT;
use crate::bootstrap::tests::wiring::{adapters_on, paced_on, record};
use crate::bootstrap::{Adapters, application};
use crate::config::Config;

/// The fetch wait (`KAFKA_FETCH_MAX_WAIT`, its minimum) of the nodes whose loop must turn fast.
const FETCH_WAIT: Duration = Duration::from_millis(50);

#[cfg(target_os = "linux")]
#[rstest::rstest]
#[case::default_keep_alive(Duration::from_secs(15))]
#[case::short(Duration::from_millis(1_500))]
fn bound_stalled_peer_sets_the_user_timeout_of_the_socket(#[case] limit: Duration) {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    let before = socket2::SockRef::from(&socket).tcp_user_timeout().unwrap();

    super::bound_stalled_peer(&socket, limit);

    let after = socket2::SockRef::from(&socket).tcp_user_timeout().unwrap();
    assert_eq!(before, None);
    assert_eq!(after, Some(limit));
}

#[cfg(target_os = "linux")]
#[rstest::rstest]
#[case::default_keep_alive(Duration::from_secs(15))]
#[case::short(Duration::from_millis(1_500))]
#[tokio::test]
async fn a_bounded_listener_gives_an_accepted_connection_the_keep_alive_as_its_user_timeout(
    #[case] keep_alive: Duration,
) {
    use axum::serve::Listener as _;
    let mut streams = config(&[]).streams;
    streams.keep_alive = keep_alive;
    let mut listener = bounded_listener(bounded(listen()).await, &streams);
    let address = listener.local_addr().unwrap();

    let client = dial(address).await;
    let (accepted, peer) = bounded(listener.accept()).await;

    assert_eq!(peer, client.local_addr().unwrap());
    let client_side = socket2::SockRef::from(&client).tcp_user_timeout().unwrap();
    let accepted_side = socket2::SockRef::from(&accepted)
        .tcp_user_timeout()
        .unwrap();
    assert_eq!(client_side, None);
    assert_eq!(accepted_side, Some(keep_alive));
}

/// A node served by `run_until` in a task of its own, the trigger of its signal and its hub.
struct Served {
    address: SocketAddr,
    hub: Arc<Hub>,
    fire: oneshot::Sender<()>,
    serving: JoinHandle<Result<Shutdown, BoxError>>,
}

impl Served {
    /// Builds the application of `adapters` with `config` and serves it until [`Served::signal`].
    async fn start(adapters: Adapters, config: Config) -> Self {
        let application = application(adapters, &config).unwrap();
        let hub = Arc::clone(&application.background.hub);
        let listener = bounded(listen()).await;
        let address = listener.local_addr().unwrap();
        let (fire, fired) = oneshot::channel::<()>();
        let signal = async move {
            drop(fired.await);
            "SIGTERM"
        };
        let serving =
            tokio::spawn(async move { run_until(application, listener, &config, signal).await });
        Self {
            address,
            hub,
            fire,
            serving,
        }
    }

    /// Sends the signal and returns the outcome of the serve sequence with the time it took.
    async fn signal(self) -> (Result<Shutdown, BoxError>, Duration) {
        let started = Instant::now();
        self.fire.send(()).unwrap();
        let outcome = bounded(self.serving).await.unwrap();
        (outcome, started.elapsed())
    }
}

/// A node on `topic` whose tail is paced with the fetch wait of `config`.
async fn paced(topic: &StaticTopic, config: Config) -> (Served, Arc<AtomicUsize>) {
    let (adapters, fetches) = paced_on(topic, config.kafka.fetch_wait);
    (Served::start(adapters, config).await, fetches)
}

async fn wait_until_closed(hub: &Hub) {
    bounded(async {
        while !hub.is_closed() {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}

fn messages(logs: &LogRecorder) -> Vec<String> {
    logs.lines().into_iter().map(|line| line.message).collect()
}

#[tokio::test]
async fn a_tail_that_panics_ends_serve_with_the_failed_task() {
    let (logs, _guard) = LogRecorder::install();
    let topic = StaticTopic::empty();
    let mut adapters = adapters_on(&topic, Box::new(topic.clone()));
    adapters.topic_probe = Arc::new(PanickingTopicProbe);
    let application = application(adapters, &config(&[])).unwrap();
    let hub = Arc::clone(&application.background.hub);
    let listener = bounded(listen()).await;

    let outcome = bounded(run_until(
        application,
        listener,
        &config(&[]),
        std::future::pending(),
    ))
    .await;

    let error = outcome.unwrap_err();
    assert_eq!(error.to_string(), "background task tail failed");
    let messages = messages(&logs);
    assert_eq!(messages.first().map(String::as_str), Some("listening"));
    assert!(
        messages.contains(&"background task failed".to_owned()),
        "{messages:?}"
    );
    assert!(!messages.contains(&"stopped".to_owned()), "{messages:?}");
    assert!(hub.is_closed());
}

#[tokio::test]
async fn an_application_on_a_static_topic_delivers_to_both_transports_and_resumes_from_the_window()
{
    let topic = StaticTopic::empty();
    let mut quick = config(&[]);
    quick.kafka.fetch_wait = FETCH_WAIT;
    let (mut adapters, fetches) = paced_on(&topic, quick.kafka.fetch_wait);
    let replay = ScriptedTopicReplay::new();
    adapters.topic_replay = Arc::new(replay.clone());
    let served = Served::start(adapters, quick).await;
    wait_until_ready(served.address).await;
    let mut sse = Sse::open(served.address, &format!("?types={TYPE}")).await;
    let mut ws = connect(served.address, &format!("?types={TYPE}")).await;

    let _first = topic.append(Some(record(1, TYPE, 0)));
    let _other = topic.append(Some(record(2, "order.paid", 0)));
    let _third = topic.append(Some(record(3, TYPE, 0)));
    sse.read_until("id: 3\n").await;
    let frames = [text(&mut ws).await, text(&mut ws).await];
    let mut resumed = Sse::open(served.address, &format!("?types={TYPE}&lastEventId=1")).await;
    resumed.read_until("id: 3\n").await;
    let mut resumed_ws = connect(served.address, &format!("?types={TYPE}&lastEventId=1")).await;
    let resumed_frame = text(&mut resumed_ws).await;
    let (idle_fetches, more) = idle_fetches(&fetches).await;
    let (outcome, _took) = served.signal().await;

    assert!(
        sse.read.contains("id: 1\nevent: user.created\ndata: {"),
        "{}",
        sse.read
    );
    assert!(!sse.read.contains("id: 2\n"), "{}", sse.read);
    assert!(frames[0].starts_with(&ws_prefix(1)), "{}", frames[0]);
    assert!(frames[1].starts_with(&ws_prefix(3)), "{}", frames[1]);
    assert!(!resumed.read.contains("id: 1\n"), "{}", resumed.read);
    assert!(resumed_frame.starts_with(&ws_prefix(3)), "{resumed_frame}");
    assert_eq!(
        replay.calls(),
        Vec::<Cursor>::new(),
        "both resumes are served from the window, never from the topic"
    );
    assert!(
        idle_fetches <= 7,
        "the paced loop spins: {idle_fetches} fetches in five fetch waits"
    );
    assert!(more > 0, "the paced loop stopped fetching");
    assert_eq!(outcome.unwrap(), Shutdown::Drained);
}

/// The fetches of the idle loop during five fetch waits (at most six of a loop that awaits each
/// wait, one more for the jitter of the timers), then the fetches until one more came (bounded).
async fn idle_fetches(fetches: &AtomicUsize) -> (usize, usize) {
    let before = fetches.load(Ordering::Relaxed);
    sleep(FETCH_WAIT.saturating_mul(5)).await;
    let after = fetches.load(Ordering::Relaxed);
    bounded(async {
        while fetches.load(Ordering::Relaxed) == after {
            sleep(FETCH_WAIT).await;
        }
    })
    .await;
    let more = fetches.load(Ordering::Relaxed).saturating_sub(after);
    (after.saturating_sub(before), more)
}

#[tokio::test]
async fn the_loop_on_an_idle_static_topic_yields_on_a_current_thread_runtime() {
    let topic = StaticTopic::empty();
    let served = Served::start(adapters_on(&topic, Box::new(topic.clone())), config(&[])).await;

    wait_until_ready(served.address).await;
    let ping = get(served.address, "/ping").await;
    let health = get(served.address, "/api/health").await;
    let (outcome, _took) = served.signal().await;

    assert_eq!(ping, (200, "pong".to_owned()));
    assert_eq!(health.0, 204, "{}", health.1);
    assert_eq!(outcome.unwrap(), Shutdown::Drained);
}

#[tokio::test]
async fn a_signal_ends_the_streams_after_their_queued_frames_and_refuses_new_subscriptions() {
    let (logs, _guard) = LogRecorder::install();
    let topic = StaticTopic::empty();
    let mut quick = config(&[]);
    quick.kafka.fetch_wait = FETCH_WAIT;
    let (served, _fetches) = paced(&topic, quick).await;
    wait_until_ready(served.address).await;
    let mut sse = Sse::open(served.address, &format!("?types={TYPE}")).await;
    let mut ws = connect(served.address, &format!("?types={TYPE}")).await;
    let mut late = dial(served.address).await;
    let head = format!(
        "GET /api/events/stream?types={TYPE} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n"
    );
    bounded(late.write_all(head.as_bytes())).await.unwrap();
    assert_eq!(get(served.address, "/ping").await.0, 200);
    let _first = topic.append(Some(record(1, TYPE, 0)));
    let _second = topic.append(Some(record(2, TYPE, 0)));
    sse.read_until("id: 2\n").await;

    let hub = Arc::clone(&served.hub);
    let address = served.address;
    let signalled = tokio::spawn(served.signal());
    wait_until_closed(&hub).await;
    sse.read_to_end().await;
    let frames = [text(&mut ws).await, text(&mut ws).await];
    let close = bounded(ws.next()).await;
    bounded(late.write_all(b"\r\n")).await.unwrap();
    let mut raw = Vec::new();
    bounded(late.read_to_end(&mut raw)).await.unwrap();
    let (outcome, _took) = bounded(signalled).await.unwrap();
    let refused = bounded(dial_refused(address)).await;

    assert!(
        sse.read.ends_with("0\r\n\r\n"),
        "the SSE body did not end: {}",
        sse.read
    );
    assert!(frames[0].starts_with(&ws_prefix(1)), "{}", frames[0]);
    assert!(frames[1].starts_with(&ws_prefix(2)), "{}", frames[1]);
    match close {
        Some(Ok(Message::Close(Some(frame)))) => assert_eq!(frame.code, CloseCode::Away),
        other => panic!("not the close frame 1001: {other:?}"),
    }
    let (status, body) = parse(&String::from_utf8(raw).unwrap());
    assert_eq!(status, 503, "{body}");
    assert!(body.contains("the node is shutting down"), "{body}");
    assert_eq!(outcome.unwrap(), Shutdown::Drained);
    assert!(refused, "new connections are refused after the signal");
    let messages = messages(&logs);
    let position = |message: &str| {
        messages
            .iter()
            .position(|logged| logged == message)
            .unwrap_or_else(|| panic!("{message} was not logged: {messages:?}"))
    };
    assert_eq!(position("listening"), 0, "{messages:?}");
    assert!(
        position("event tail ready") < position("shutdown signal received"),
        "{messages:?}"
    );
    assert!(
        position("shutdown signal received") < position("subscription rejected"),
        "{messages:?}"
    );
    assert_eq!(
        messages.last().map(String::as_str),
        Some("stopped"),
        "{messages:?}"
    );
}

#[expect(
    clippy::disallowed_methods,
    reason = "the tests are clients of the in-process node"
)]
async fn dial_refused(address: SocketAddr) -> bool {
    tokio::net::TcpStream::connect(address)
        .await
        .is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionRefused)
}

/// The events of a stall: 200 of 60 kB, far more than the socket buffers of a client that takes
/// no data hold.
const STALL_EVENTS: u64 = 200;

/// The grace period of the node whose shutdown is forced: shorter than [`STREAM_DRAIN_TIMEOUT`].
const SHORT_GRACE: Duration = Duration::from_millis(300);

/// A ready node with the grace period `grace`, whose subscribers queue every event of a stall,
/// and its topic. A subscriber that takes no data never falls
/// behind: its session still has queued frames to write when the hub closes, whatever the timing
/// of the threads, so it cannot reach its close frame while its client reads nothing.
async fn stalling_node(grace: Duration) -> (Served, StaticTopic) {
    let topic = StaticTopic::empty();
    let mut stalling = config(&[]);
    stalling.kafka.fetch_wait = FETCH_WAIT;
    stalling.streams.buffer = usize::try_from(STALL_EVENTS).unwrap();
    stalling.shutdown_grace_period = grace;
    let (adapters, _fetches) = paced_on(&topic, FETCH_WAIT);
    let served = Served::start(adapters, stalling).await;
    wait_until_ready(served.address).await;
    (served, topic)
}

/// Appends the events of a stall and waits until the tail of the node at `address` handed all of
/// them to the hub (the status reports the position at the end of the round that published them).
async fn stall(topic: &StaticTopic, address: SocketAddr) {
    for n in 1..=STALL_EVENTS {
        let _offset = topic.append(Some(record(n, TYPE, 60_000)));
    }
    wait_for_status(address, &format!(r#""position":"{STALL_EVENTS}""#)).await;
}

/// A node whose WebSocket client takes no data while the node holds far more for it than the
/// socket buffers take; returns the node after the tail handed every event to the hub.
async fn stalled_client(grace: Duration) -> (Served, Client) {
    let (served, topic) = stalling_node(grace).await;
    let client = small_window_client(served.address).await;
    stall(&topic, served.address).await;
    (served, client)
}

/// A WebSocket client of [`TYPE`] with a receive buffer of a few kilobytes, which it never reads.
async fn small_window_client(address: SocketAddr) -> Client {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.set_recv_buffer_size(4_096).unwrap();
    let stream = bounded(socket.connect(address)).await.unwrap();
    let request = format!("ws://{address}/api/events/ws?types={TYPE}")
        .into_client_request()
        .unwrap();
    let (client, _response) = bounded(tokio_tungstenite::client_async(request, stream))
        .await
        .unwrap();
    client
}

/// Every message the client still receives, each read bounded, and the error that ended the
/// stream (`None` when it ended without one).
async fn remaining(client: &mut Client) -> (Vec<Message>, Option<tungstenite::Error>) {
    let mut messages = Vec::new();
    loop {
        match bounded(client.next()).await {
            Some(Ok(message)) => messages.push(message),
            Some(Err(error)) => return (messages, Some(error)),
            None => return (messages, None),
        }
    }
}

/// The runtimes as `start` builds them: the node on a multi-thread runtime that `serve_on` ends,
/// its client on a current-thread runtime of the test that outlives it.
#[test]
fn a_client_that_takes_no_data_is_cut_without_its_close_frame_when_the_sequence_ends() {
    let runtime = serving_runtime().unwrap();
    let client_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (served, topic) = runtime.block_on(stalling_node(Duration::from_secs(5)));
    let address = served.address;
    let mut client = client_runtime.block_on(small_window_client(address));
    runtime.block_on(stall(&topic, address));
    let hub = Arc::clone(&served.hub);

    // The sequence waits the whole drain bound of a node for the stalled stream; the real sockets
    // of this test cannot run on paused time, so it waits the 2 s of STREAM_DRAIN_TIMEOUT.
    let ((outcome, took), left) = serve_on(runtime, async move {
        let signalled = served.signal().await;
        (signalled, hub.subscriber_count())
    });
    let (messages, error) = client_runtime.block_on(remaining(&mut client));

    assert_eq!(outcome.unwrap(), Shutdown::Drained);
    assert!(took >= STREAM_DRAIN_TIMEOUT, "{took:?}");
    assert_eq!(left, 1, "the stalled stream ended before the runtime");
    let closes = messages.iter().filter(|message| message.is_close()).count();
    assert_eq!(
        closes,
        0,
        "a close frame among {} messages, ended by {error:?}",
        messages.len()
    );
}

#[tokio::test]
async fn a_grace_period_shorter_than_the_drain_timeout_forces_the_shutdown() {
    let (served, _client) = stalled_client(SHORT_GRACE).await;

    // The real sockets of this test wait the whole grace period.
    let (outcome, took) = served.signal().await;

    assert_eq!(outcome.unwrap(), Shutdown::Forced);
    assert!(took >= SHORT_GRACE, "{took:?}");
    assert!(took < STREAM_DRAIN_TIMEOUT, "{took:?}");
}
