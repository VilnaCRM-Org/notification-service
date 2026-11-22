//! The keep-alive comment of the byte streams of the event stream (FR-5, FR-17).
//!
//! The timer is lazy: a chunk only records when it was written and never touches the timer. The
//! timer is armed for the last write plus the interval; when it fires after a later write it is
//! armed again, once, for that write plus the interval, and only when no chunk was written for the
//! whole interval does it write the comment. So the comment comes exactly when the interval passed
//! without a chunk, as a timer restarted by every chunk would write it, at one timer operation
//! per interval instead of one per chunk. A poll polls the timer at most twice and loops nowhere.

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::Bytes;
use futures_util::stream::{BoxStream, Stream, StreamExt as _};
use tokio::time::{Instant, Sleep, sleep_until};

/// The keep-alive comment: the comment line `:` and the empty line that ends the block.
pub const COMMENT: &str = ":\n\n";

/// The chunks of `chunks`, with [`COMMENT`] whenever `interval` passes without a chunk, counted
/// from now (tokio time, which tests can pause). It ends when the chunks end.
pub fn with_keep_alive<S>(chunks: S, interval: Duration) -> impl Stream<Item = Bytes> + Send
where
    S: Stream<Item = Bytes> + Send + 'static,
{
    KeepAlive::new(chunks.boxed(), interval)
}

/// The stream of [`with_keep_alive`].
struct KeepAlive {
    chunks: BoxStream<'static, Bytes>,
    interval: Duration,
    /// When the last chunk or comment was written.
    last_write: Instant,
    /// True when a chunk was written since the timer was last armed.
    written: bool,
    timer: Pin<Box<Sleep>>,
}

impl KeepAlive {
    fn new(chunks: BoxStream<'static, Bytes>, interval: Duration) -> Self {
        let last_write = Instant::now();
        Self {
            chunks,
            interval,
            last_write,
            written: false,
            timer: Box::pin(sleep_until(due(last_write, interval))),
        }
    }
}

impl Stream for KeepAlive {
    type Item = Bytes;

    /// The chunks first, so a chunk that is ready is never held back by the comment; then the
    /// timer. When it fired after a chunk, it is armed for that chunk plus the interval and polled
    /// once more, which registers the waker or, when that moment passed too, writes the comment;
    /// a fired timer always writes the comment and is armed for one interval from now.
    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Bytes>> {
        let this = self.get_mut();
        if let Poll::Ready(chunk) = this.chunks.poll_next_unpin(context) {
            this.last_write = Instant::now();
            this.written = true;
            return Poll::Ready(chunk);
        }
        if this.timer.as_mut().poll(context).is_pending() {
            return Poll::Pending;
        }
        if this.written {
            this.written = false;
            this.timer
                .as_mut()
                .reset(due(this.last_write, this.interval));
            if this.timer.as_mut().poll(context).is_pending() {
                return Poll::Pending;
            }
        }
        this.last_write = Instant::now();
        this.timer
            .as_mut()
            .reset(due(this.last_write, this.interval));
        Poll::Ready(Some(Bytes::from_static(COMMENT.as_bytes())))
    }
}

/// The moment `interval` after `from`; `from` itself in the unreachable case that it overflows
/// the clock (the interval is at most minutes).
fn due(from: Instant, interval: Duration) -> Instant {
    from.checked_add(interval).unwrap_or(from)
}

#[cfg(test)]
mod tests;
