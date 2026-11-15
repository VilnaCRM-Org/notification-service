//! The bound of the SSE client: a read that never completes fails within [`LIMIT`] with what
//! arrived, however often the stream writes. Time is paused and the stream is in memory.

use std::time::Duration;

use tokio::io::{AsyncWriteExt as _, DuplexStream};
use tokio::time::Instant;

use super::Sse;
use crate::limit::LIMIT;

/// The head of an SSE response.
const HEAD: &str =
    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n";

/// The pause between two writes of a stream: far shorter than the limit, so a bound that each
/// write started again would never end the read.
const PAUSE: Duration = Duration::from_secs(1);

/// `text` as one chunk of a chunked body.
fn chunk(text: &str) -> String {
    format!("{:x}\r\n{text}\r\n", text.len())
}

/// Writes `head`, then `repeated` every [`PAUSE`] until the client goes away.
async fn feed(mut server: DuplexStream, head: &str, repeated: String) {
    if server.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    while server.write_all(repeated.as_bytes()).await.is_ok() {
        tokio::time::sleep(PAUSE).await;
    }
}

/// Runs `read` against a stream that writes `head` and then `repeated` every [`PAUSE`], and
/// returns the message it failed with and how long it took. A read that does not fail within
/// twice the limit fails the test.
async fn failure_of<R, F>(head: &'static str, repeated: String, read: R) -> (String, Duration)
where
    R: FnOnce(DuplexStream) -> F,
    F: Future<Output = ()> + Send + 'static,
{
    let (client, server) = tokio::io::duplex(1_024);
    let feeding = tokio::spawn(feed(server, head, repeated));
    let started = Instant::now();
    let reading = tokio::spawn(read(client));
    let ended = tokio::time::timeout(LIMIT.saturating_mul(2), reading).await;
    let elapsed = started.elapsed();
    feeding.abort();
    let failure = ended
        .expect("the read ends within the limit")
        .expect_err("the read fails");
    let message = failure.into_panic().downcast::<String>().unwrap();
    (*message, elapsed)
}

/// Reads the head of the response, then one event.
async fn one_event(client: DuplexStream) {
    Sse::over(client).await.events(1).await;
}

/// Reads the head of the response, then up to one keep-alive.
async fn one_keep_alive(client: DuplexStream) {
    Sse::over(client).await.keep_alives(1).await;
}

/// Reads the head of the response.
async fn head(client: DuplexStream) {
    Sse::over(client).await;
}

#[tokio::test(start_paused = true)]
async fn a_read_of_events_that_sees_only_keep_alives_fails_within_the_limit() {
    let (message, elapsed) = failure_of(HEAD, chunk(":\n\n"), one_event).await;

    assert!(
        message.starts_with("1 events did not arrive within 30s: 0 events [] and "),
        "{message}"
    );
    assert!(message.ends_with(" keep-alives did"), "{message}");
    assert!(elapsed >= LIMIT, "{elapsed:?}");
}

#[tokio::test(start_paused = true)]
async fn a_read_of_keep_alives_that_sees_only_events_fails_within_the_limit() {
    let event = chunk("id: 7\nevent: user.created\ndata: {\"id\":\"e\"}\n\n");
    let (message, elapsed) = failure_of(HEAD, event, one_keep_alive).await;

    assert!(
        message.starts_with("1 keep-alives did not arrive within 30s: "),
        "{message}"
    );
    assert!(message.contains(" events [7, 7"), "{message}");
    assert!(message.ends_with("and 0 keep-alives did"), "{message}");
    assert!(elapsed >= LIMIT, "{elapsed:?}");
}

#[tokio::test(start_paused = true)]
async fn a_head_that_never_ends_fails_within_the_limit() {
    let header = "x-padding: 1\r\n".to_owned();
    let (message, elapsed) = failure_of("HTTP/1.1 200 OK\r\n", header, head).await;

    assert!(
        message.starts_with("no response head arrived within 30s: HTTP/1.1 200 OK\r\nx-padding: 1"),
        "{message}"
    );
    assert!(elapsed >= LIMIT, "{elapsed:?}");
}
