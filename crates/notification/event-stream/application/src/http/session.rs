//! The state of one WebSocket connection without its socket and without a timer: the commands a
//! client sends (FR-10, FR-11) and the subscription it reads by position (FR-12 to FR-15, FR-24).
//!
//! A text frame is a command, checked in the order size, JSON and shape, names, limit of the set;
//! an invalid one is answered with one error frame and changes nothing. The pump of
//! [`super::socket`] owns the socket, the pings and the deadlines.

use std::collections::BTreeSet;

use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, close_code};
use event_stream_domain::{Cursor, EventType};

use super::dto::{Command, TypesError, parse_names};
use super::frame::error_json;
use super::subscription::{EndReason, Step, Subscription, TooManyTypes};

/// The reason of the error frame of a text that is not a command.
const INVALID_COMMAND: &str = "invalid command";

/// The reason of the error frame of a binary frame.
const BINARY: &str = "binary frames are not supported";

/// The reason of the error frame of a text above [`Session::MAX_COMMAND_BYTES`].
const TOO_LARGE: &str = "the command is too large";

/// What the pump does after an inbound message.
#[derive(Debug)]
pub(crate) enum Inbound {
    /// Nothing to write.
    Continue,
    /// Write this frame.
    Reply(Message),
    /// The client closed the connection.
    Close,
}

/// One WebSocket connection: its subscription and the commands that change it.
#[derive(Debug)]
pub(crate) struct Session {
    subscription: Subscription,
}

impl Session {
    /// The largest command, in bytes: the largest valid command, an unsubscribe of 100 names of
    /// 128 characters, takes 13 134 bytes.
    pub(crate) const MAX_COMMAND_BYTES: usize = 16_384;

    /// The session of a subscription that the hub registered.
    pub(crate) const fn new(subscription: Subscription) -> Self {
        Self { subscription }
    }

    /// Handles one inbound message: a text is a command, a binary frame is answered with an
    /// error frame, a ping or a pong changes nothing (the socket answers a ping itself) and a
    /// close frame closes the session.
    pub(crate) fn on_message(&mut self, message: Message) -> Inbound {
        match message {
            Message::Text(text) => match self.command(text.as_str()) {
                Ok(()) => Inbound::Continue,
                Err(reason) => Inbound::Reply(error_frame(reason)),
            },
            Message::Binary(_bytes) => Inbound::Reply(error_frame(BINARY)),
            Message::Ping(_payload) | Message::Pong(_payload) => Inbound::Continue,
            Message::Close(_frame) => Inbound::Close,
        }
    }

    /// One step of the subscription, which may wait for a permit and read a Kafka page: for a
    /// turn that polls it to its end.
    pub(crate) async fn step(&mut self) -> Step {
        self.subscription.step().await
    }

    /// One step of the subscription that never reads Kafka, for a turn that inbound frames and
    /// pings may cancel (see [`Subscription::step_live`]).
    pub(crate) async fn live_step(&mut self) -> Step {
        self.subscription.step_live().await
    }

    /// True when the next step reads Kafka: a permit wait and one page read, which no inbound
    /// frame may interrupt.
    pub(crate) fn replaying(&self) -> bool {
        self.subscription.replaying()
    }

    /// The close frame of an ended subscription: 1001 when the hub closed, 1011 when a replay
    /// could not read Kafka (FR-11).
    pub(crate) const fn close_message(reason: EndReason) -> Message {
        let code = match reason {
            EndReason::HubClosed => close_code::AWAY,
            EndReason::TopicFailed => close_code::ERROR,
        };
        Message::Close(Some(CloseFrame {
            code,
            reason: Utf8Bytes::from_static(""),
        }))
    }

    /// The number of event types the connection holds.
    pub(crate) fn type_count(&self) -> usize {
        self.subscription.type_count()
    }

    /// The highest id settled for the connection.
    pub(crate) const fn position(&self) -> Cursor {
        self.subscription.position()
    }

    /// The error chain of the read that ended the replay, once a step ended with
    /// [`EndReason::TopicFailed`].
    pub(crate) fn failure(&self) -> Option<&str> {
        self.subscription.failure()
    }

    /// Applies one command; the error is the reason of its error frame.
    fn command(&mut self, text: &str) -> Result<(), &'static str> {
        if text.len() > Self::MAX_COMMAND_BYTES {
            return Err(TOO_LARGE);
        }
        let command = serde_json::from_str::<Command>(text).map_err(|_invalid| INVALID_COMMAND)?;
        match command {
            Command::Subscribe { types } => self
                .subscription
                .subscribe(event_types(&types)?)
                .map_err(|_limit| TooManyTypes::MESSAGE),
            Command::Unsubscribe { types } => {
                self.subscription.unsubscribe(&event_types(&types)?);
                Ok(())
            }
        }
    }
}

/// The distinct event types of a command; the error is the reason of its error frame.
fn event_types(names: &[String]) -> Result<BTreeSet<EventType>, &'static str> {
    parse_names(names.iter().map(String::as_str)).map_err(TypesError::reason)
}

/// The error frame `{"error":"<reason>"}`.
fn error_frame(reason: &str) -> Message {
    Message::Text(Utf8Bytes::from(error_json(reason)))
}

#[cfg(test)]
mod tests;
