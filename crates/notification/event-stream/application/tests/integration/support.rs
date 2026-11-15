//! A started node of the event stream served on a loopback port of its own, behind the body limit
//! the service applies to every route, and a WebSocket client of it.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use event_stream_application::http::{EventStreamState, StreamSettings, router};
use event_stream_application::{
    EventPublisher as _, Hub, HubSettings, ReplaySettings, StatusSource, Subscriptions, TailState,
    Window,
};
use event_stream_domain::{Cursor, TopicReplay};
use futures_util::{SinkExt as _, StreamExt as _};
use test_support::bus::FakeQueryBus;
use test_support::fixture::stored_event;
use test_support::static_topic::StaticTopic;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{WebSocketStream, client_async};

/// The limit of every awaited call of a test; the sockets are real, so the clock is too.
pub(crate) const LIMIT: Duration = Duration::from_secs(10);

/// The keep-alive of a node whose test must see no ping.
pub(crate) const QUIET: Duration = Duration::from_secs(60);

/// The position at which every node starts, and the floor of its window.
pub(crate) const START: u64 = 900;

/// The request body limit of a node (`HTTP_MAX_BODY_BYTES` in the service), far below one
/// WebSocket message, so that a test reading a larger message shows the limit does not apply.
pub(crate) const BODY_LIMIT: usize = 1_024;

/// A WebSocket client over a plain TCP stream.
pub(crate) type Client = WebSocketStream<TcpStream>;

/// The settings of a node.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Settings {
    /// The queue of each subscriber.
    pub(crate) buffer: usize,
    /// The ping period and write deadline.
    pub(crate) keep_alive: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            buffer: 64,
            keep_alive: QUIET,
        }
    }
}

/// A started node at [`START`] whose routes are served on `address` behind [`BODY_LIMIT`].
pub(crate) struct Node {
    pub(crate) hub: Arc<Hub>,
    window: Arc<Window>,
    _tail: watch::Sender<TailState>,
    pub(crate) address: SocketAddr,
    _server: JoinHandle<()>,
}

/// Serves a started node with `settings` and an empty topic for its replays.
pub(crate) async fn node(settings: Settings) -> Node {
    node_with(settings, Arc::new(StaticTopic::empty())).await
}

/// Serves a started node with `settings` whose replays read `replay`.
pub(crate) async fn node_with(settings: Settings, replay: Arc<dyn TopicReplay>) -> Node {
    let hub = Hub::new(HubSettings {
        max_subscribers: 16,
        buffer: settings.buffer,
    });
    hub.start(Cursor::new(START));
    let window = Arc::new(Window::new(1_000, usize::MAX));
    window.restart(Cursor::new(START));
    let (tail, receiver) = watch::channel(TailState {
        started: true,
        ..TailState::default()
    });
    let subscriptions = Subscriptions::new(
        Arc::clone(&hub),
        Arc::clone(&window),
        replay,
        receiver.clone(),
        ReplaySettings {
            max_events: 1_000_000,
            max_data_bytes: 65_536,
            permits: 4,
        },
    );
    let state = EventStreamState {
        query_bus: Arc::new(FakeQueryBus::unregistered()),
        subscriptions: Arc::new(subscriptions),
        status: Arc::new(StatusSource::new(
            receiver,
            Arc::clone(&window),
            Arc::<Hub>::clone(&hub),
        )),
        hub: Arc::clone(&hub),
        settings: StreamSettings {
            keep_alive: settings.keep_alive,
        },
    };
    let (routes, _document) = router::<EventStreamState>().split_for_parts();
    let listener = listen().await;
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let app = routes
            .layer(DefaultBodyLimit::max(BODY_LIMIT))
            .with_state(state);
        axum::serve(listener, app).await.unwrap();
    });
    Node {
        hub,
        window,
        _tail: tail,
        address,
        _server: server,
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "integration tests open the sockets their subject talks to"
)]
async fn listen() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").await.unwrap()
}

#[expect(
    clippy::disallowed_methods,
    reason = "integration tests open the sockets their subject talks to"
)]
async fn dial(address: SocketAddr) -> TcpStream {
    bounded(TcpStream::connect(address)).await.unwrap()
}

/// Awaits `future` within [`LIMIT`].
pub(crate) async fn bounded<F: Future>(future: F) -> F::Output {
    timeout(LIMIT, future)
        .await
        .expect("the call must end within the limit")
}

/// The data of the event `id`.
pub(crate) fn event_json(id: u64) -> String {
    format!(r#"{{"n":{id}}}"#)
}

/// The WebSocket frame of the event `id` of `event_type`.
pub(crate) fn frame(id: u64, event_type: &str) -> String {
    format!(
        r#"{{"id":"{id}","type":"{event_type}","data":{}}}"#,
        event_json(id)
    )
}

impl Node {
    /// What the tail does with a valid event: the window first, then the hub.
    pub(crate) fn pass(&self, id: u64, event_type: &str) {
        let event = stored_event(id, event_type, &event_json(id));
        self.window.push(Arc::new(event.clone()));
        self.hub.publish(&event);
    }

    /// Opens a WebSocket on `/events/ws` with `query` (empty or starting with `?`) and, when
    /// given, the `Last-Event-ID` header; returns the client and the status of the handshake.
    pub(crate) async fn connect(&self, query: &str, last_event_id: Option<&str>) -> (Client, u16) {
        let mut request = format!("ws://{}/events/ws{query}", self.address)
            .into_client_request()
            .unwrap();
        if let Some(id) = last_event_id {
            request
                .headers_mut()
                .insert("last-event-id", id.parse().unwrap());
        }
        let stream = dial(self.address).await;
        let (client, response) = bounded(client_async(request, stream)).await.unwrap();
        (client, response.status().as_u16())
    }
}

/// The next message of the client.
pub(crate) async fn next(client: &mut Client) -> Option<Result<Message, WsError>> {
    bounded(client.next()).await
}

/// The text of the next message, which must be a text frame.
pub(crate) async fn text(client: &mut Client) -> String {
    match next(client).await {
        Some(Ok(Message::Text(text))) => text.as_str().to_owned(),
        other => panic!("not a text frame: {other:?}"),
    }
}

/// The texts of the next `count` messages, which must be text frames.
pub(crate) async fn texts(client: &mut Client, count: usize) -> Vec<String> {
    let mut texts = Vec::with_capacity(count);
    for _ in 0..count {
        texts.push(text(client).await);
    }
    texts
}

/// Sends one message.
pub(crate) async fn send(client: &mut Client, message: Message) {
    bounded(client.send(message)).await.unwrap();
}

/// Sends a ping and waits for its pong: the server reads inbound frames in order, so every frame
/// sent before the ping was handled once the pong arrives.
pub(crate) async fn barrier(client: &mut Client) {
    send(client, Message::Ping("barrier".into())).await;
    match next(client).await {
        Some(Ok(Message::Pong(payload))) => assert_eq!(payload.as_ref(), b"barrier"),
        other => panic!("not the pong of the barrier: {other:?}"),
    }
}
