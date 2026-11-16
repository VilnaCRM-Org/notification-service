//! The pump of one WebSocket connection: it writes the frames of the session, a ping every
//! keep-alive period, the error frames of invalid commands and the close frame of an ended
//! subscription (FR-9 to FR-11, FR-24).
//!
//! Every write has the keep-alive period as its deadline: `WebSocket::send` has none, and its
//! flush stays pending while the peer reads nothing. A turn that reads Kafka (a replay turn) is
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
use tokio::task::coop::consume_budget;
use tokio::time::{Interval, MissedTickBehavior, interval, timeout};

use super::session::{Inbound, Session};
use super::subscription::{EndReason, Step, log_replay_failed};

/// What an inbound read gives: a message, a transport error, or `None` once the connection ended.
type Received = Option<Result<Message, axum::Error>>;

/// The socket side of a WebSocket connection, so that the pump can run over a scripted one.
pub(crate) trait Connection: Send {
    /// The next inbound message; `None` once the connection ended. Cancel safe.
    fn receive(&mut self) -> impl Future<Output = Received> + Send;

    /// Writes and flushes one message.
    fn transmit(
        &mut self,
        message: Message,
    ) -> impl Future<Output = Result<(), axum::Error>> + Send;
}

impl Connection for WebSocket {
    fn receive(&mut self) -> impl Future<Output = Received> + Send {
        self.recv()
    }

    fn transmit(
        &mut self,
        message: Message,
    ) -> impl Future<Output = Result<(), axum::Error>> + Send {
        self.send(message)
    }
}

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
            Step::Frame(frame) => self.send(Message::Text(frame.json.clone())).await,
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
/// command applies to every frame not yet written. The live step never reads Kafka: when the
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

/// Writes one message within `deadline`: a transport error ends the connection, a missed deadline
/// evicts the subscriber.
async fn send<C: Connection>(
    connection: &mut C,
    deadline: Duration,
    message: Message,
) -> ControlFlow<Stop> {
    match timeout(deadline, connection.transmit(message)).await {
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
