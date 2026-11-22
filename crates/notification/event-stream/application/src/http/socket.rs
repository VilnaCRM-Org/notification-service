//! The pump of one WebSocket connection: it writes the frames of the session, a ping every
//! keep-alive period, the error frames of invalid commands and the close frame of an ended
//! subscription (FR-9 to FR-11, FR-24).
//!
//! Every write has the keep-alive period as its deadline: a flush has none of its own, and stays
//! pending while the peer reads nothing. A frame is written with the frames that are already
//! ready after it, at most [`MAX_BATCH_FRAMES`] and up to [`MAX_BATCH_BYTES`]: each is fed to the
//! socket and the batch is flushed once, so a burst of events costs one write to the peer and not
//! one per frame. Each feed and the flush has its own deadline, so a client that takes data more
//! slowly than a whole batch per period but takes some every period is never evicted (FR-24).
//! Before it takes a further frame, a batch handles every inbound message that already arrived,
//! without awaiting one, so a command applies to every frame not yet written (FR-10); the floor
//! and type rules apply to every frame. A turn that reads Kafka (a replay turn) is
//! polled to its end without reading an inbound frame, because a cancelled page read would re-enter
//! the permit queue at its end and read the page again; it still writes the pings. A live turn
//! never starts a Kafka read. Tungstenite writes the reply to a client's close frame only on its
//! next read, so after a close the pump keeps reading for up to [`CLOSE_TIMEOUT`].

use std::ops::ControlFlow;
use std::pin::pin;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use event_stream_domain::Cursor;
use futures_util::{FutureExt as _, SinkExt};
use tokio::task::coop::consume_budget;
use tokio::time::error::Elapsed;
use tokio::time::{Interval, MissedTickBehavior, interval, timeout};

use super::frame::Frame;
use super::session::{Inbound, Session};
use super::subscription::{EndReason, Step, log_replay_failed};

/// What an inbound read gives: a message, a transport error, or `None` once the connection ended.
type Received = Option<Result<Message, axum::Error>>;

/// The socket side of a WebSocket connection, so that the pump can run over a scripted one.
pub(crate) trait Connection: Send {
    /// The next inbound message; `None` once the connection ended. Cancel safe.
    fn receive(&mut self) -> impl Future<Output = Received> + Send;

    /// Buffers one message: it reaches the peer with the next flush, or earlier once the buffer
    /// passes its write size.
    fn feed(&mut self, message: Message) -> impl Future<Output = Result<(), axum::Error>> + Send;

    /// Writes every buffered message to the peer.
    fn flush(&mut self) -> impl Future<Output = Result<(), axum::Error>> + Send;
}

impl Connection for WebSocket {
    fn receive(&mut self) -> impl Future<Output = Received> + Send {
        self.recv()
    }

    fn feed(&mut self, message: Message) -> impl Future<Output = Result<(), axum::Error>> + Send {
        SinkExt::feed(self, message)
    }

    fn flush(&mut self) -> impl Future<Output = Result<(), axum::Error>> + Send {
        SinkExt::flush(self)
    }
}

/// The most frames one write batch holds.
pub(crate) const MAX_BATCH_FRAMES: usize = 16;

/// The size up to which a write batch takes another ready frame, in bytes; the frame that crosses
/// it is the last of its batch. The write buffer of the connection has the same size
/// ([`super::handler::WRITE_BUFFER_BYTES`]), so a batch stays in it until its flush, or until the
/// frame that crosses the size, and never holds more than that frame on top (NFR-9).
pub(crate) const MAX_BATCH_BYTES: usize = 16_384;

/// How long the pump waits for the client's close frame after its own, or for the client to hang
/// up after the client's close frame.
pub(crate) const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

/// Why the turns of a pump stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// The connection ended or failed: nothing more is written.
    Gone,
    /// A write missed its deadline.
    Stalled,
    /// The client sent a close frame.
    ClientClosed,
    /// The subscription ended.
    Ended(EndReason),
}

/// Which branch of a live turn completed first.
enum Event {
    /// The ping period passed.
    Ping,
    /// An inbound read completed.
    Inbound(Received),
    /// A step of the session completed.
    Step(Step),
}

/// One connection, its session and its ping timer.
struct Pump<C> {
    connection: C,
    session: Session,
    keep_alive: Duration,
    ping: Interval,
}

/// Serves one connection until it ends: one task per connection. `keep_alive` is the ping period
/// and the deadline of every write. When it returns, the session and its subscriber are dropped,
/// which frees the slot.
pub(crate) async fn serve<C: Connection>(connection: C, session: Session, keep_alive: Duration) {
    let mut ping = interval(keep_alive);
    ping.set_missed_tick_behavior(MissedTickBehavior::Delay);
    ping.reset();
    let mut pump = Pump {
        connection,
        session,
        keep_alive,
        ping,
    };
    let stop = pump.run().await;
    pump.finish(stop).await;
}

impl<C: Connection> Pump<C> {
    /// Runs turns until one stops: a replay turn when the next step reads Kafka, else a live turn.
    async fn run(&mut self) -> Stop {
        loop {
            let turn = if self.session.replaying() {
                self.replay_turn().await
            } else {
                self.live_turn().await
            };
            if let ControlFlow::Break(stop) = turn {
                return stop;
            }
        }
    }

    /// Polls one step to its end; the pings of the period are written meanwhile and no inbound
    /// frame is read.
    async fn replay_turn(&mut self) -> ControlFlow<Stop> {
        let step = {
            let mut step = pin!(self.session.step());
            loop {
                tokio::select! {
                    biased;
                    _tick = self.ping.tick() => {
                        send(&mut self.connection, self.keep_alive, ping()).await?;
                    }
                    step = &mut step => break step,
                }
            }
        };
        self.on_step(step).await
    }

    /// Waits for the first of a ping, an inbound message and a step, and handles it.
    async fn live_turn(&mut self) -> ControlFlow<Stop> {
        match pick(&mut self.connection, &mut self.session, &mut self.ping).await {
            Event::Ping => self.send(ping()).await,
            Event::Inbound(received) => self.on_inbound(received).await,
            Event::Step(step) => self.on_step(step).await,
        }
    }

    /// An inbound message goes to the session; an ended or failed read ends the connection.
    async fn on_inbound(&mut self, received: Received) -> ControlFlow<Stop> {
        let Some(Ok(message)) = received else {
            return ControlFlow::Break(Stop::Gone);
        };
        match self.session.on_message(message) {
            Inbound::Continue => ControlFlow::Continue(()),
            Inbound::Reply(frame) => self.send(frame).await,
            Inbound::Close => ControlFlow::Break(Stop::ClientClosed),
        }
    }

    /// A frame is written; an idle step spends one unit of the task's cooperative budget, so a
    /// chain of steps that awaits nothing still hands the thread back to the scheduler.
    async fn on_step(&mut self, step: Step) -> ControlFlow<Stop> {
        match step {
            Step::Frame(frame) => self.write_batch(&frame).await,
            Step::Idle => {
                consume_budget().await;
                ControlFlow::Continue(())
            }
            Step::End(reason) => ControlFlow::Break(Stop::Ended(reason)),
        }
    }

    async fn send(&mut self, message: Message) -> ControlFlow<Stop> {
        send(&mut self.connection, self.keep_alive, message).await
    }

    /// Feeds the text of `first` and of the frames that are ready after it, while the batch holds
    /// fewer than [`MAX_BATCH_FRAMES`] and fewer than [`MAX_BATCH_BYTES`], then flushes once; each
    /// feed and the flush within its own deadline. Before each further frame the inbound messages
    /// that already arrived are handled, so a command applies to the rest of the batch; one that
    /// ends the connection ends the batch unflushed.
    async fn write_batch(&mut self, first: &Frame) -> ControlFlow<Stop> {
        self.feed(first).await?;
        let mut frames = 1_usize;
        let mut bytes = first.json().len();
        while frames < MAX_BATCH_FRAMES && bytes < MAX_BATCH_BYTES {
            self.on_arrived().await?;
            let Some(frame) = self.session.try_next() else {
                break;
            };
            frames = frames.saturating_add(1);
            bytes = bytes.saturating_add(frame.json().len());
            self.feed(&frame).await?;
        }
        outcome(timeout(self.keep_alive, self.connection.flush()).await)
    }

    /// Handles every inbound message that already arrived, without awaiting one; a message that
    /// ends the connection stops at once.
    async fn on_arrived(&mut self) -> ControlFlow<Stop> {
        while let Some(received) = self.connection.receive().now_or_never() {
            self.on_inbound(received).await?;
        }
        ControlFlow::Continue(())
    }

    /// Feeds the text of `frame` within the keep-alive period.
    async fn feed(&mut self, frame: &Frame) -> ControlFlow<Stop> {
        outcome(timeout(self.keep_alive, self.connection.feed(text(frame))).await)
    }

    /// What follows the last turn: an evicted subscriber is logged, a client close frame and the
    /// close frame of an ended subscription are followed by the lingering.
    async fn finish(&mut self, stop: Stop) {
        match stop {
            Stop::Gone => {}
            Stop::Stalled => log_evicted(self.session.type_count(), self.session.position()),
            Stop::ClientClosed => self.linger().await,
            Stop::Ended(reason) => self.close(reason).await,
        }
    }

    /// Logs a failed replay, writes the close frame of `reason` and, when it was written,
    /// lingers.
    async fn close(&mut self, reason: EndReason) {
        if let Some(error) = self.session.failure() {
            log_replay_failed("ws", error);
        }
        if self
            .send(Session::close_message(reason))
            .await
            .is_continue()
        {
            self.linger().await;
        }
    }

    /// Reads and drops inbound messages until the connection ends or fails, at most for
    /// [`CLOSE_TIMEOUT`].
    async fn linger(&mut self) {
        let _drained = timeout(CLOSE_TIMEOUT, drain(&mut self.connection)).await;
    }
}

/// The first of a ping, an inbound message and a live step; every branch is cancel safe. The
/// order is fixed: the ping first, so that it is sent on time, then the inbound message, so that a
/// command applies to every frame not yet written; within a batch, [`Pump::write_batch`] handles
/// the inbound messages that arrived before each further frame. The live step never reads Kafka: when the
/// window evicted past the floor after the turn asked `replaying()`, it idles, and the next turn
/// is a replay turn.
async fn pick<C: Connection>(
    connection: &mut C,
    session: &mut Session,
    ping: &mut Interval,
) -> Event {
    tokio::select! {
        biased;
        _tick = ping.tick() => Event::Ping,
        received = connection.receive() => Event::Inbound(received),
        step = session.live_step() => Event::Step(step),
    }
}

/// Writes one message within `deadline`.
async fn send<C: Connection>(
    connection: &mut C,
    deadline: Duration,
    message: Message,
) -> ControlFlow<Stop> {
    let write = async {
        connection.feed(message).await?;
        connection.flush().await
    };
    outcome(timeout(deadline, write).await)
}

/// The text message of a frame: a clone of its shared bytes.
fn text(frame: &Frame) -> Message {
    Message::Text(frame.json().clone())
}

/// What a write within its deadline means for the pump: a transport error ends the connection, a
/// missed deadline evicts the subscriber.
fn outcome(written: Result<Result<(), axum::Error>, Elapsed>) -> ControlFlow<Stop> {
    match written {
        Ok(Ok(())) => ControlFlow::Continue(()),
        Ok(Err(_transport)) => ControlFlow::Break(Stop::Gone),
        Err(_elapsed) => ControlFlow::Break(Stop::Stalled),
    }
}

async fn drain<C: Connection>(connection: &mut C) {
    while let Some(Ok(_message)) = connection.receive().await {}
}

/// The ping of the keep-alive period, without a payload.
const fn ping() -> Message {
    Message::Ping(Bytes::new())
}

fn log_evicted(types: usize, position: Cursor) {
    let last_id = position.value();
    tracing::warn!(transport = "ws", types, last_id, "subscriber evicted");
}

#[cfg(test)]
mod tests;
