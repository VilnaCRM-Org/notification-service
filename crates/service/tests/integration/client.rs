//! Plain HTTP and SSE clients of a node on a loopback port; every await is bounded by [`LIMIT`],
//! a read of several events or keep-alives as a whole.

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
        loop {
            self.decode();
            if let Some(end) = self.text.find("\n\n") {
                let block: String = self.text.drain(..end.saturating_add(2)).collect();
                return parse_block(block.trim_end_matches('\n'));
            }
            assert!(!self.ended, "the stream ended: {}", self.text);
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

#[cfg(test)]
mod tests;
