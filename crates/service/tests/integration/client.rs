//! Plain HTTP, SSE, GraphQL event-stream and raw WebSocket handshake clients of a node on a
//! loopback port; every await is bounded by [`LIMIT`], a read of several events, blocks,
//! keep-alives or comments as a whole.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

use crate::limit::{LIMIT, bounded, dial};
use crate::support::Answer;

/// The pause between two reads of the status route of a node.
const POLL: Duration = Duration::from_millis(50);

/// The loopback address of a served binary that listens on `port`.
pub(crate) fn local(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

/// The query of a subscription of `event_type`.
pub(crate) fn subscription(event_type: &str) -> String {
    format!("?types={event_type}")
}

/// Sends a `GET` with `connection: close` and reads the whole answer.
pub(crate) async fn get(address: SocketAddr, path: &str) -> Answer {
    let mut stream = dial(address).await;
    let request = format!("GET {path} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n");
    bounded(stream.write_all(request.as_bytes())).await.unwrap();
    let mut raw = Vec::new();
    bounded(stream.read_to_end(&mut raw)).await.unwrap();
    Answer::parse(&String::from_utf8(raw).unwrap())
}

/// The body of the status route of the node at `address`.
pub(crate) async fn status(address: SocketAddr) -> Value {
    let answer = get(address, "/api/events/status").await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    serde_json::from_str(&answer.body).unwrap()
}

/// Polls the status route until its member `field` equals `expected`, within [`LIMIT`], and returns
/// that status.
pub(crate) async fn wait_for(address: SocketAddr, field: &str, expected: &Value) -> Value {
    let waited = tokio::time::timeout(LIMIT, async {
        loop {
            let status = status(address).await;
            if status[field] == *expected {
                return status;
            }
            tokio::time::sleep(POLL).await;
        }
    })
    .await;
    waited.unwrap_or_else(|_elapsed| panic!("{field} never became {expected}"))
}

/// One event of an SSE stream.
#[derive(Debug)]
pub(crate) struct Frame {
    /// The cursor of the event: its offset plus 1.
    pub(crate) id: u64,
    /// The event type.
    pub(crate) event: String,
    /// The event object.
    pub(crate) data: Value,
}

/// A block of an SSE body.
#[derive(Debug)]
enum Item {
    Event(Frame),
    KeepAlive,
}

/// What one read of an SSE body collected so far.
#[derive(Debug, Default)]
struct Collected {
    events: Vec<Frame>,
    keep_alives: usize,
}

/// An open SSE response: the connection, the bytes of the chunked body not yet decoded and the
/// decoded text not yet split into blocks.
#[derive(Debug)]
pub(crate) struct Sse<S = TcpStream> {
    stream: S,
    raw: Vec<u8>,
    text: String,
    /// Whether the final chunk was read.
    ended: bool,
}

impl Sse {
    /// Sends the subscription request of `query`, with `Last-Event-ID` when `last_event_id` is
    /// given, and reads its head, which must open a stream.
    pub(crate) async fn open(address: SocketAddr, query: &str, last_event_id: Option<u64>) -> Self {
        let mut stream = dial(address).await;
        let cursor =
            last_event_id.map_or_else(String::new, |id| format!("last-event-id: {id}\r\n"));
        let request = format!(
            "GET /api/events/stream{query} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n{cursor}\r\n"
        );
        bounded(stream.write_all(request.as_bytes())).await.unwrap();
        Self::over(stream).await
    }
}

impl<S: AsyncRead + Unpin> Sse<S> {
    /// Reads the response head on `stream` within [`LIMIT`]; the head must open a stream.
    async fn over(stream: S) -> Self {
        let mut sse = Self {
            stream,
            raw: Vec::new(),
            text: String::new(),
            ended: false,
        };
        let read = tokio::time::timeout(LIMIT, sse.read_head()).await;
        let head = read
            .unwrap_or_else(|_elapsed| {
                let raw = String::from_utf8_lossy(&sse.raw);
                panic!("no response head arrived within {LIMIT:?}: {raw}")
            })
            .to_lowercase();
        assert!(head.starts_with("http/1.1 200"), "{head}");
        assert!(head.contains("content-type: text/event-stream"), "{head}");
        assert!(head.contains("transfer-encoding: chunked"), "{head}");
        sse
    }

    /// The next `count` events, all within [`LIMIT`]; the keep-alive comments between them are
    /// skipped.
    pub(crate) async fn events(&mut self, count: usize) -> Vec<Frame> {
        let wanted = format!("{count} events");
        self.read(&wanted, |collected| collected.events.len() >= count)
            .await
            .events
    }

    /// Reads until `count` keep-alive comments came, within [`LIMIT`], and returns the events read
    /// meanwhile.
    pub(crate) async fn keep_alives(&mut self, count: usize) -> Vec<Frame> {
        let wanted = format!("{count} keep-alives");
        self.read(&wanted, |collected| collected.keep_alives >= count)
            .await
            .events
    }

    /// Reads blocks until `enough` holds, within [`LIMIT`] for the whole read; past it the test
    /// fails with what arrived.
    async fn read(
        &mut self,
        wanted: &str,
        enough: impl Fn(&Collected) -> bool + Send + Sync,
    ) -> Collected {
        let mut collected = Collected::default();
        let read = tokio::time::timeout(LIMIT, self.collect(&mut collected, &enough)).await;
        assert!(
            read.is_ok(),
            "{wanted} did not arrive within {LIMIT:?}: {} events {:?} and {} keep-alives did",
            collected.events.len(),
            ids(&collected.events),
            collected.keep_alives,
        );
        collected
    }

    /// Adds the blocks of the body to `collected` until `enough` holds.
    async fn collect(
        &mut self,
        collected: &mut Collected,
        enough: &(impl Fn(&Collected) -> bool + Sync),
    ) {
        while !enough(collected) {
            match self.next_item().await {
                Item::Event(frame) => collected.events.push(frame),
                Item::KeepAlive => collected.keep_alives = collected.keep_alives.saturating_add(1),
            }
        }
    }

    /// Reads the response head; what follows it stays in `raw`.
    async fn read_head(&mut self) -> String {
        loop {
            if let Some(end) = find(&self.raw, b"\r\n\r\n") {
                let head = String::from_utf8(self.raw[..end].to_vec()).unwrap();
                self.raw.drain(..end.saturating_add(4));
                return head;
            }
            self.receive().await;
        }
    }

    /// The next block of the body.
    async fn next_item(&mut self) -> Item {
        let block = self.next_text().await;
        let block = block.unwrap_or_else(|| panic!("the stream ended: {}", self.text));
        parse_block(&block)
    }

    /// The text of the next block of the body without its blank line, or `None` once the final
    /// chunk was read and no whole block is left.
    async fn next_text(&mut self) -> Option<String> {
        loop {
            self.decode();
            if let Some(end) = self.text.find("\n\n") {
                let block: String = self.text.drain(..end.saturating_add(2)).collect();
                return Some(block.trim_end_matches('\n').to_owned());
            }
            if self.ended {
                return None;
            }
            self.receive().await;
        }
    }

    /// Reads what the connection holds into `raw`; the connection must not close. Every caller
    /// bounds the whole read it belongs to.
    async fn receive(&mut self) {
        let mut buffer = [0_u8; 8_192];
        let count = self.stream.read(&mut buffer).await.unwrap();
        assert_ne!(
            count,
            0,
            "the connection closed: {}",
            String::from_utf8_lossy(&self.raw)
        );
        self.raw.extend_from_slice(&buffer[..count]);
    }

    /// Moves every complete chunk of `raw` into `text`; the chunk of size 0 ends the body.
    fn decode(&mut self) {
        while let Some(line) = find(&self.raw, b"\r\n") {
            let size =
                usize::from_str_radix(std::str::from_utf8(&self.raw[..line]).unwrap(), 16).unwrap();
            let start = line.saturating_add(2);
            let end = start.saturating_add(size);
            if self.raw.len() < end.saturating_add(2) {
                return;
            }
            if size == 0 {
                self.ended = true;
            }
            self.text
                .push_str(std::str::from_utf8(&self.raw[start..end]).unwrap());
            self.raw.drain(..end.saturating_add(2));
        }
    }
}

/// The position of the first `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// An event block (`id`, `event` and `data` lines) or a keep-alive comment (lines of `:`).
fn parse_block(block: &str) -> Item {
    if block.lines().all(|line| line.starts_with(':')) {
        return Item::KeepAlive;
    }
    Item::Event(Frame {
        id: field(block, "id: ").parse().unwrap(),
        event: field(block, "event: ").to_owned(),
        data: serde_json::from_str(field(block, "data: ")).unwrap(),
    })
}

/// The value of the line of `block` that starts with `name`.
fn field<'block>(block: &'block str, name: &str) -> &'block str {
    block
        .lines()
        .find_map(|line| line.strip_prefix(name))
        .unwrap_or_else(|| panic!("no {name} line in {block}"))
}

/// The cursors of `frames`.
pub(crate) fn ids(frames: &[Frame]) -> Vec<u64> {
    frames.iter().map(|frame| frame.id).collect()
}

/// The envelope ids of `frames`: the `id` of each event object.
pub(crate) fn envelope_ids(frames: &[Frame]) -> Vec<String> {
    frames
        .iter()
        .map(|frame| frame.data["id"].as_str().unwrap().to_owned())
        .collect()
}

/// A block of a GraphQL event stream other than a comment (graphql-sse's distinct connections
/// mode).
#[derive(Debug, PartialEq)]
pub(crate) enum Block {
    /// `event: next`, its `id:` line and its execution result.
    Next {
        /// The event id of a subscription result; `None` for a single result.
        id: Option<u64>,
        /// The execution result of the `data:` line.
        result: Value,
    },
    /// `event: complete` with its bare `data:` line.
    Complete,
}

/// A block of a GraphQL event stream as read: a comment, or another block.
#[derive(Debug)]
enum Parsed {
    Comment,
    Block(Block),
}

/// What one read of a GraphQL event stream collected so far.
#[derive(Debug, Default)]
struct Received {
    /// The blocks other than comments, in order.
    blocks: Vec<Block>,
    /// The comments.
    comments: usize,
}

impl Received {
    /// Counts a comment or keeps another block.
    fn add(&mut self, parsed: Parsed) {
        match parsed {
            Parsed::Comment => self.comments = self.comments.saturating_add(1),
            Parsed::Block(block) => self.blocks.push(block),
        }
    }
}

/// An open GraphQL event stream of the POST form, requested as the graphql-sse client requests
/// it; every read is bounded by [`LIMIT`] as a whole.
#[derive(Debug)]
pub(crate) struct GraphqlSse {
    sse: Sse,
}

impl GraphqlSse {
    /// Posts `request` (see [`post_event_stream`]) and reads the head, which must open an event
    /// stream.
    pub(crate) async fn open(
        address: SocketAddr,
        request: &Value,
        last_event_id: Option<u64>,
    ) -> Self {
        let stream = post_event_stream(address, request, last_event_id).await;
        Self {
            sse: Sse::over(stream).await,
        }
    }

    /// Posts `request` like [`GraphqlSse::open`] and reads the whole answer, which must not open
    /// an event stream: the connection closes after it.
    pub(crate) async fn refused(
        address: SocketAddr,
        request: &Value,
        last_event_id: Option<u64>,
    ) -> Answer {
        let mut stream = post_event_stream(address, request, last_event_id).await;
        let mut raw = Vec::new();
        bounded(stream.read_to_end(&mut raw)).await.unwrap();
        Answer::parse(&String::from_utf8(raw).unwrap())
    }

    /// The next `count` blocks other than comments, all within [`LIMIT`]; the comments between
    /// them are skipped.
    pub(crate) async fn blocks(&mut self, count: usize) -> Vec<Block> {
        let wanted = format!("{count} blocks");
        self.read(&wanted, |received| received.blocks.len() >= count)
            .await
            .blocks
    }

    /// Reads until `count` comments came, within [`LIMIT`], and returns the other blocks read
    /// meanwhile.
    pub(crate) async fn comments(&mut self, count: usize) -> Vec<Block> {
        let wanted = format!("{count} comments");
        self.read(&wanted, |received| received.comments >= count)
            .await
            .blocks
    }

    /// Reads up to the end of the body, within [`LIMIT`], and returns the blocks other than
    /// comments; the body must end after a whole block.
    pub(crate) async fn rest(mut self) -> Vec<Block> {
        let mut received = Received::default();
        let read = tokio::time::timeout(LIMIT, async {
            while let Some(block) = self.sse.next_text().await {
                received.add(parse_graphql_block(&block));
            }
        })
        .await;
        assert!(
            read.is_ok(),
            "the stream did not end within {LIMIT:?}: {:?} and {} comments arrived",
            received.blocks,
            received.comments,
        );
        assert!(
            self.sse.text.is_empty(),
            "the stream ended inside a block: {}",
            self.sse.text
        );
        received.blocks
    }

    /// Reads blocks until `enough` holds, within [`LIMIT`] for the whole read; past it the test
    /// fails with what arrived.
    async fn read(
        &mut self,
        wanted: &str,
        enough: impl Fn(&Received) -> bool + Send + Sync,
    ) -> Received {
        let mut received = Received::default();
        let read = tokio::time::timeout(LIMIT, self.collect(&mut received, &enough)).await;
        assert!(
            read.is_ok(),
            "{wanted} did not arrive within {LIMIT:?}: {:?} and {} comments did",
            received.blocks,
            received.comments,
        );
        received
    }

    /// Adds the blocks of the body to `received` until `enough` holds; the body must not end
    /// before.
    async fn collect(
        &mut self,
        received: &mut Received,
        enough: &(impl Fn(&Received) -> bool + Sync),
    ) {
        while !enough(received) {
            let block = self.sse.next_text().await;
            let block =
                block.unwrap_or_else(|| panic!("the stream ended after {:?}", received.blocks));
            received.add(parse_graphql_block(&block));
        }
    }
}

/// Sends a POST of `request` to the GraphQL path as the graphql-sse client sends it:
/// `content-type: application/json`, exactly `accept: text/event-stream`, and `last-event-id` when
/// `last_event_id` is given.
async fn post_event_stream(
    address: SocketAddr,
    request: &Value,
    last_event_id: Option<u64>,
) -> TcpStream {
    let mut stream = dial(address).await;
    let body = request.to_string();
    let length = body.len();
    let cursor = last_event_id.map_or_else(String::new, |id| format!("last-event-id: {id}\r\n"));
    let head = format!(
        "POST /api/graphql HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\ncontent-type: application/json\r\naccept: text/event-stream\r\n{cursor}content-length: {length}\r\n\r\n{body}"
    );
    bounded(stream.write_all(head.as_bytes())).await.unwrap();
    stream
}

/// A comment (lines of `:`), a `complete` block (exactly `event: complete` and a bare `data:`), or
/// a `next` block: `event: next`, the `id:` line of a subscription result, then one `data:` line of
/// JSON. Any other block fails the test.
fn parse_graphql_block(block: &str) -> Parsed {
    if block.lines().all(|line| line.starts_with(':')) {
        return Parsed::Comment;
    }
    if block == "event: complete\ndata:" {
        return Parsed::Block(Block::Complete);
    }
    let lines: Vec<&str> = block.lines().collect();
    let (id, data) = match lines.as_slice() {
        ["event: next", data] => (None, *data),
        ["event: next", id, data] => (Some(event_id(id, block)), *data),
        _ => panic!("not a block of a GraphQL event stream: {block:?}"),
    };
    let result = data
        .strip_prefix("data: ")
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_else(|| panic!("no data: line of JSON in {block:?}"));
    Parsed::Block(Block::Next { id, result })
}

/// The event id of the `id:` line `line` of `block`: a decimal number without leading zeros.
fn event_id(line: &str, block: &str) -> u64 {
    line.strip_prefix("id: ")
        .and_then(|text| text.parse::<u64>().ok().filter(|id| id.to_string() == text))
        .unwrap_or_else(|| panic!("no id: line of a decimal event id in {block:?}"))
}

/// The `Sec-WebSocket-Key` of every handshake: the sample nonce of RFC 6455.
const WEBSOCKET_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

/// Opens a WebSocket subscriber of `event_type` with a raw handshake, which must be answered
/// `101`. The connection it returns is a subscriber of the node until it is dropped; it reads
/// nothing, so it answers no ping.
pub(crate) async fn upgrade(address: SocketAddr, event_type: &str) -> TcpStream {
    let (stream, head, _rest) = handshake(address, event_type).await;
    assert!(head.starts_with("http/1.1 101"), "{head}");
    assert!(head.contains("upgrade: websocket"), "{head}");
    stream
}

/// Sends the WebSocket handshake of a subscription of `event_type`, which the node must refuse,
/// and reads the answer: its head and its body of `content-length` bytes. The handshake asks for
/// `connection: upgrade`, so the connection stays open after the answer.
pub(crate) async fn upgrade_refused(address: SocketAddr, event_type: &str) -> Answer {
    let (mut stream, head, mut body) = handshake(address, event_type).await;
    assert!(!head.starts_with("http/1.1 101"), "{head}");
    let length: usize = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .unwrap_or_else(|| panic!("no content-length in {head}"))
        .parse()
        .unwrap();
    bounded(async {
        while body.len() < length {
            receive_into(&mut stream, &mut body).await;
        }
    })
    .await;
    Answer::parse(&format!(
        "{head}\r\n\r\n{}",
        String::from_utf8(body).unwrap()
    ))
}

/// Sends the WebSocket handshake of a subscription of `event_type` (`upgrade`, `connection`,
/// `sec-websocket-key` and `sec-websocket-version` 13) and reads the head of the answer, within
/// [`LIMIT`]. Returns the connection, the head in lower case and the bytes read after it.
async fn handshake(address: SocketAddr, event_type: &str) -> (TcpStream, String, Vec<u8>) {
    let mut stream = dial(address).await;
    let query = subscription(event_type);
    let request = format!(
        "GET /api/events/ws{query} HTTP/1.1\r\nhost: localhost\r\nupgrade: websocket\r\nconnection: upgrade\r\nsec-websocket-key: {WEBSOCKET_KEY}\r\nsec-websocket-version: 13\r\n\r\n"
    );
    bounded(stream.write_all(request.as_bytes())).await.unwrap();
    let mut raw = Vec::new();
    let head = bounded(head_of(&mut stream, &mut raw)).await;
    (stream, head, raw)
}

/// Reads `stream` into `raw` up to the end of a response head and returns the head in lower case;
/// the bytes after it stay in `raw`. The caller bounds the read.
async fn head_of(stream: &mut TcpStream, raw: &mut Vec<u8>) -> String {
    loop {
        if let Some(end) = find(raw, b"\r\n\r\n") {
            let head = String::from_utf8(raw[..end].to_vec()).unwrap();
            raw.drain(..end.saturating_add(4));
            return head.to_lowercase();
        }
        receive_into(stream, raw).await;
    }
}

/// Reads what `stream` holds onto the end of `raw`; the connection must not close. The caller
/// bounds the read.
async fn receive_into(stream: &mut TcpStream, raw: &mut Vec<u8>) {
    let mut buffer = [0_u8; 1_024];
    let count = stream.read(&mut buffer).await.unwrap();
    assert_ne!(
        count,
        0,
        "the connection closed: {}",
        String::from_utf8_lossy(raw)
    );
    raw.extend_from_slice(&buffer[..count]);
}

#[cfg(test)]
mod tests;
