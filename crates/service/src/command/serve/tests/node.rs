//! A node served in process on a loopback port and plain HTTP, SSE and WebSocket clients of it;
//! its tail runs behind the pacing adapter of `bootstrap::tests::wiring`.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::StreamExt as _;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Instant, sleep, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, client_async};

use crate::cli::ServeArgs;
use crate::config::Config;

/// The limit of every awaited call; the sockets are real, so the clock is too.
pub(super) const LIMIT: Duration = Duration::from_secs(10);

/// The event type every test subscribes to.
pub(super) const TYPE: &str = "user.created";

/// A WebSocket client over a plain TCP stream.
pub(super) type Client = WebSocketStream<TcpStream>;

/// The configuration of a test node: the defaults with the given flags.
pub(super) fn config(flags: &[&str]) -> Config {
    use clap::Parser as _;
    let arguments = ["service", "serve"].iter().chain(flags);
    let crate::cli::Command::Serve(args) = crate::cli::Cli::parse_from(arguments).command else {
        panic!("`serve` arguments expected");
    };
    let args = ServeArgs {
        aws_region: Some("us-east-1".to_owned()),
        ..args
    };
    Config::from_args(args).unwrap()
}

#[expect(
    clippy::disallowed_methods,
    reason = "the in-process node of these tests is served on a socket of its own"
)]
pub(super) async fn listen() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").await.unwrap()
}

#[expect(
    clippy::disallowed_methods,
    reason = "the tests are clients of the in-process node"
)]
pub(super) async fn dial(address: SocketAddr) -> TcpStream {
    bounded(TcpStream::connect(address)).await.unwrap()
}

/// Awaits `future` within [`LIMIT`].
pub(super) async fn bounded<F: Future>(future: F) -> F::Output {
    timeout(LIMIT, future)
        .await
        .expect("the call must end within the limit")
}

/// Sends a `GET` with `connection: close` and returns the status and the body of the answer.
pub(super) async fn get(address: SocketAddr, path: &str) -> (u16, String) {
    let mut stream = dial(address).await;
    let head = format!("GET {path} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n");
    bounded(stream.write_all(head.as_bytes())).await.unwrap();
    let mut raw = Vec::new();
    bounded(stream.read_to_end(&mut raw)).await.unwrap();
    parse(&String::from_utf8(raw).unwrap())
}

/// The status and the body of a raw answer; a chunked body is returned with its chunk lines.
pub(super) fn parse(raw: &str) -> (u16, String) {
    let (head, body) = raw.split_once("\r\n\r\n").unwrap();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    (status, body.to_owned())
}

/// Polls the status route until the node is ready, within [`LIMIT`].
pub(super) async fn wait_until_ready(address: SocketAddr) {
    wait_for_status(address, r#""state":"ready""#).await;
}

/// Polls the status route until its body holds `needle`, within [`LIMIT`].
pub(super) async fn wait_for_status(address: SocketAddr, needle: &str) {
    let started = Instant::now();
    loop {
        let (status, body) = get(address, "/api/events/status").await;
        assert_eq!(status, 200, "{body}");
        if body.contains(needle) {
            return;
        }
        assert!(
            started.elapsed() < LIMIT,
            "the status never held {needle}: {body}"
        );
        sleep(Duration::from_millis(20)).await;
    }
}

/// An open SSE response: the connection and everything read from it so far.
pub(super) struct Sse {
    stream: TcpStream,
    pub(super) read: String,
}

impl Sse {
    /// Sends the subscription request of `query` and reads its head.
    pub(super) async fn open(address: SocketAddr, query: &str) -> Self {
        let mut stream = dial(address).await;
        let head = format!(
            "GET /api/events/stream{query} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n"
        );
        bounded(stream.write_all(head.as_bytes())).await.unwrap();
        let mut sse = Self {
            stream,
            read: String::new(),
        };
        sse.read_until("\r\n\r\n").await;
        sse
    }

    /// Reads until what was read holds `needle`.
    pub(super) async fn read_until(&mut self, needle: &str) {
        let mut buffer = [0_u8; 4_096];
        while !self.read.contains(needle) {
            let count = bounded(self.stream.read(&mut buffer)).await.unwrap();
            assert_ne!(count, 0, "the stream ended before {needle}: {}", self.read);
            self.read
                .push_str(std::str::from_utf8(&buffer[..count]).unwrap());
        }
    }

    /// Reads to the end of the response.
    pub(super) async fn read_to_end(&mut self) {
        let mut rest = Vec::new();
        bounded(self.stream.read_to_end(&mut rest)).await.unwrap();
        self.read.push_str(&String::from_utf8(rest).unwrap());
    }
}

/// Opens a WebSocket on the route with `query`.
pub(super) async fn connect(address: SocketAddr, query: &str) -> Client {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
    let request = format!("ws://{address}/api/events/ws{query}")
        .into_client_request()
        .unwrap();
    let stream = dial(address).await;
    let (client, response) = bounded(client_async(request, stream)).await.unwrap();
    assert_eq!(response.status().as_u16(), 101);
    client
}

/// The text of the next message, which must be a text frame.
pub(super) async fn text(client: &mut Client) -> String {
    match bounded(client.next()).await {
        Some(Ok(Message::Text(text))) => text.as_str().to_owned(),
        other => panic!("not a text frame: {other:?}"),
    }
}

/// The start of the WebSocket frame of the event with id `sequence` of [`TYPE`], up to its data.
pub(super) fn ws_prefix(sequence: u64) -> String {
    format!(r#"{{"id":"{sequence}","type":"{TYPE}","data":{{"id":"#)
}
