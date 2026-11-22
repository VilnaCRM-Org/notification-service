use std::future::poll_fn;
use std::pin::{Pin, pin};
use std::task::Poll;
use std::time::Duration;

use axum::body::Bytes;
use futures_util::stream::{self, Stream, StreamExt as _};
use tokio::sync::mpsc;
use tokio::time::advance;

use super::{COMMENT, with_keep_alive};

/// The interval of these tests; not the default of 15 s, so a lost setting shows.
const INTERVAL: Duration = Duration::from_secs(7);

/// One millisecond less than [`INTERVAL`].
const JUST_BEFORE_INTERVAL: Duration = Duration::from_millis(6_999);

const ONE_MILLISECOND: Duration = Duration::from_millis(1);

/// The polls [`within_polls`] allows, each followed by a yield to the scheduler.
const POLLS: usize = 16;

type Chunks = Pin<Box<dyn Stream<Item = Bytes> + Send>>;

/// The keep-alive stream over the chunks sent to the returned sender; it ends when the sender is
/// dropped.
fn kept_alive() -> (mpsc::UnboundedSender<Bytes>, Chunks) {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let chunks = stream::poll_fn(move |context| receiver.poll_recv(context));
    (sender, Box::pin(with_keep_alive(chunks, INTERVAL)))
}

/// Polls `future` at most [`POLLS`] times with a yield after each pending poll.
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

/// The next item as text; panics when the stream is still pending.
async fn next(stream: &mut Chunks) -> Option<String> {
    within_polls(stream.next())
        .await
        .expect("an item within the polls")
        .map(|chunk| String::from_utf8(chunk.to_vec()).unwrap())
}

/// True when the stream writes nothing while the clock moves on by `quiet`.
async fn silent(stream: &mut Chunks, quiet: Duration) -> bool {
    let before = within_polls(stream.next()).await.is_none();
    advance(quiet).await;
    before && within_polls(stream.next()).await.is_none()
}

#[test]
fn the_comment_is_the_comment_line_and_the_empty_line() {
    assert_eq!(COMMENT, ":\n\n");
}

#[tokio::test(start_paused = true)]
async fn without_a_chunk_the_comment_comes_after_every_interval() {
    let (_sender, mut stream) = kept_alive();

    assert!(silent(&mut stream, JUST_BEFORE_INTERVAL).await);
    advance(ONE_MILLISECOND).await;
    assert_eq!(next(&mut stream).await.as_deref(), Some(COMMENT));
    assert!(silent(&mut stream, JUST_BEFORE_INTERVAL).await);
    advance(ONE_MILLISECOND).await;
    assert_eq!(next(&mut stream).await.as_deref(), Some(COMMENT));
}

#[tokio::test(start_paused = true)]
async fn the_chunks_pass_unchanged_in_order_and_the_stream_ends_with_them() {
    let (sender, mut stream) = kept_alive();

    sender.send(Bytes::from_static(b"one")).unwrap();
    sender.send(Bytes::from_static(b"two")).unwrap();
    drop(sender);

    assert_eq!(next(&mut stream).await.as_deref(), Some("one"));
    assert_eq!(next(&mut stream).await.as_deref(), Some("two"));
    assert_eq!(next(&mut stream).await, None);
}

#[tokio::test(start_paused = true)]
async fn every_chunk_restarts_the_interval() {
    let (sender, mut stream) = kept_alive();

    assert!(silent(&mut stream, Duration::from_secs(3)).await);
    sender.send(Bytes::from_static(b"one")).unwrap();
    assert_eq!(next(&mut stream).await.as_deref(), Some("one"));
    assert!(silent(&mut stream, Duration::from_secs(5)).await);
    sender.send(Bytes::from_static(b"two")).unwrap();
    assert_eq!(next(&mut stream).await.as_deref(), Some("two"));

    assert!(silent(&mut stream, JUST_BEFORE_INTERVAL).await);
    advance(ONE_MILLISECOND).await;
    assert_eq!(next(&mut stream).await.as_deref(), Some(COMMENT));
}

#[tokio::test(start_paused = true)]
async fn a_comment_also_restarts_the_interval() {
    let (sender, mut stream) = kept_alive();
    advance(INTERVAL).await;
    assert_eq!(next(&mut stream).await.as_deref(), Some(COMMENT));

    assert!(silent(&mut stream, Duration::from_secs(4)).await);
    sender.send(Bytes::from_static(b"one")).unwrap();
    assert_eq!(next(&mut stream).await.as_deref(), Some("one"));
    assert!(silent(&mut stream, JUST_BEFORE_INTERVAL).await);
    advance(ONE_MILLISECOND).await;
    assert_eq!(next(&mut stream).await.as_deref(), Some(COMMENT));
}

#[tokio::test(start_paused = true)]
async fn a_chunk_that_is_ready_comes_before_a_due_comment() {
    let (sender, mut stream) = kept_alive();
    advance(INTERVAL).await;

    sender.send(Bytes::from_static(b"one")).unwrap();

    assert_eq!(next(&mut stream).await.as_deref(), Some("one"));
    assert!(silent(&mut stream, JUST_BEFORE_INTERVAL).await);
    advance(ONE_MILLISECOND).await;
    assert_eq!(next(&mut stream).await.as_deref(), Some(COMMENT));
}

#[tokio::test(start_paused = true)]
async fn a_timer_that_fires_after_a_chunk_stays_pending_until_the_interval_after_the_chunk() {
    let (sender, mut stream) = kept_alive();
    advance(Duration::from_secs(2)).await;
    sender.send(Bytes::from_static(b"one")).unwrap();
    assert_eq!(next(&mut stream).await.as_deref(), Some("one"));

    advance(Duration::from_secs(5)).await;
    let at_the_first_deadline = within_polls(stream.next()).await.is_none();
    advance(Duration::from_millis(1_999)).await;
    let before_the_second = within_polls(stream.next()).await.is_none();
    advance(ONE_MILLISECOND).await;

    assert!(at_the_first_deadline);
    assert!(before_the_second);
    assert_eq!(next(&mut stream).await.as_deref(), Some(COMMENT));
}

#[tokio::test(start_paused = true)]
async fn a_poll_after_the_interval_that_follows_a_chunk_writes_the_comment_at_once() {
    let (sender, mut stream) = kept_alive();
    advance(Duration::from_secs(1)).await;
    sender.send(Bytes::from_static(b"one")).unwrap();
    assert_eq!(next(&mut stream).await.as_deref(), Some("one"));

    advance(Duration::from_secs(19)).await;

    assert_eq!(next(&mut stream).await.as_deref(), Some(COMMENT));
    assert!(silent(&mut stream, JUST_BEFORE_INTERVAL).await);
    advance(ONE_MILLISECOND).await;
    assert_eq!(next(&mut stream).await.as_deref(), Some(COMMENT));
}
