//! The GraphQL subscription of the event stream: the per-request link between a subscription
//! transport and the `events` field, the `Event` object, built in the subscriber's task from the
//! frame that the hub shares (NFR-1), and the subscription root.
//!
//! The field checks its arguments, registers one hub subscriber and reports through the link
//! whether the hub took or refused it. Its stream takes no step until the transport releases the
//! link, so nothing reads the window or the broker before the head (FR-9, NFR-2); then it yields
//! one `Event` per frame and hands each event id to the link (FR-10).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_graphql::{Context, Json, SimpleObject};
use axum::http::HeaderValue;
use event_stream_domain::Sequence;
use futures_util::stream::{self, Stream, StreamExt as _};
use serde::Deserialize;
use shared_application::graphql::problem_to_error;
use shared_application::http::Problem;
use tokio::sync::watch;

use super::state;
use crate::http::EventStreamState;
use crate::http::dto::{CursorFault, parse_names, read_cursor, types_problem};
use crate::http::frame::Frame;
use crate::http::hub::HubError;
use crate::http::subscription::{Subscription, SubscriptionRequest, log_replay_failed};

/// The transport that `replay failed` names for a GraphQL subscription (FR-19 (c), FR-22).
const TRANSPORT: &str = "graphql-sse";

/// Where the subscription of one request stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamPhase {
    /// No subscriber registered yet; also the phase of every query.
    Pending,
    /// The `events` field registered its hub subscriber.
    Opened,
    /// The hub refused the subscriber; nothing is registered.
    Refused(HubError),
    /// The transport wrote the head; the subscription may take its first step.
    Released,
}

/// The per-request link between a subscription transport and the `events` field. The transport
/// puts one clone into the request data and keeps the other.
#[derive(Debug, Clone)]
pub struct StreamLink {
    inner: Arc<LinkInner>,
}

#[derive(Debug)]
struct LinkInner {
    /// The `Last-Event-ID` header of the request, read by the field (FR-11).
    last_event_id: Option<HeaderValue>,
    /// `Pending` -> `Opened` -> `Released`, or `Pending` -> `Refused`.
    phase: watch::Sender<StreamPhase>,
    /// The event id behind the last result the field yielded; 0 when none.
    delivered: AtomicU64,
}

impl StreamLink {
    /// A `Pending` link carrying the `Last-Event-ID` header of the request.
    #[must_use]
    pub fn new(last_event_id: Option<HeaderValue>) -> Self {
        Self {
            inner: Arc::new(LinkInner {
                last_event_id,
                phase: watch::Sender::new(StreamPhase::Pending),
                delivered: AtomicU64::new(0),
            }),
        }
    }

    /// The current phase.
    #[must_use]
    pub fn phase(&self) -> StreamPhase {
        *self.inner.phase.borrow()
    }

    /// Completes once the phase is not `Pending`: the field registered its subscriber or the hub
    /// refused it.
    pub async fn decided(&self) {
        self.wait_for(|phase| *phase != StreamPhase::Pending).await;
    }

    /// Turns `Opened` into `Released`, which lets the subscription take its first step; called by
    /// the transport once it decided to write the head. Any other phase stays as it is.
    pub fn release(&self) {
        self.inner.phase.send_if_modified(|phase| {
            let opened = *phase == StreamPhase::Opened;
            if opened {
                *phase = StreamPhase::Released;
            }
            opened
        });
    }

    /// The id of the last yielded result, once: a second call before the next result answers
    /// `None`, and so does a call before the first one.
    #[must_use]
    pub fn take_delivered(&self) -> Option<Sequence> {
        Sequence::new(self.inner.delivered.swap(0, Ordering::Relaxed)).ok()
    }

    /// The `Last-Event-ID` header of the request.
    pub(crate) fn last_event_id(&self) -> Option<&HeaderValue> {
        self.inner.last_event_id.as_ref()
    }

    /// The field registered its hub subscriber.
    pub(crate) fn opened(&self) {
        self.inner.phase.send_replace(StreamPhase::Opened);
    }

    /// The hub refused the subscriber with `error`.
    pub(crate) fn refused(&self, error: HubError) {
        self.inner.phase.send_replace(StreamPhase::Refused(error));
    }

    /// Completes once the phase is `Released`.
    pub(crate) async fn released(&self) {
        self.wait_for(|phase| *phase == StreamPhase::Released).await;
    }

    /// The field yielded the result of the event `sequence`. The whole response stream is polled
    /// in the connection's task, so the writer and the reader are one task and `Relaxed` suffices.
    pub(crate) fn delivered(&self, sequence: Sequence) {
        self.inner
            .delivered
            .store(sequence.value(), Ordering::Relaxed);
    }

    /// Completes once `done` holds for the phase; the sender lives as long as the link, so the
    /// wait ends only through `done`.
    async fn wait_for(&self, done: impl FnMut(&StreamPhase) -> bool) {
        let mut phase = self.inner.phase.subscribe();
        drop(phase.wait_for(done).await);
    }
}

/// An event of a subscribed type: the same object as a WebSocket frame.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
#[graphql(name = "Event")]
pub struct EventObject {
    /// Event id: the Kafka offset of the event plus one, in decimal; the cursor of a resume.
    pub id: String,
    /// Event type name.
    #[graphql(name = "type")]
    pub event_type: String,
    /// The event object the producer wrote: envelope id, type, occurredAt and data.
    // More than half of the complexity limit 1 000: a document selects `data` at most once (FR-1).
    #[graphql(complexity = 501)]
    pub data: Json<async_graphql::Value>,
}

/// The member `data` of a frame's WebSocket text; serde skips its members `id` and `type`.
#[derive(Debug, Deserialize)]
struct FrameData {
    /// The event object, its members in the producer's order whatever the features of
    /// `serde_json`.
    data: async_graphql::Value,
}

impl From<&Frame> for EventObject {
    /// `id` from the sequence, `type` from the event type and `data` from the member `data` of the
    /// WebSocket text. A text that cannot be read gives a `null` `data`; `Frame::encode` never
    /// writes one, because it wraps the stored event's JSON.
    fn from(frame: &Frame) -> Self {
        Self {
            id: frame.sequence.to_string(),
            event_type: frame.event_type.as_str().to_owned(),
            data: Json(
                serde_json::from_str::<FrameData>(frame.json.as_str())
                    .map_or(async_graphql::Value::Null, |read| read.data),
            ),
        }
    }
}

/// The request was run without the [`StreamLink`] that every subscription transport adds.
#[derive(Debug, thiserror::Error)]
#[error("the GraphQL request has no StreamLink data")]
struct MissingLink;

/// The link of the request; its absence is a wiring fault, logged once by `Problem::internal`.
fn link<'ctx>(ctx: &Context<'ctx>) -> async_graphql::Result<&'ctx StreamLink> {
    ctx.data::<StreamLink>()
        .map_err(|_error| problem_to_error(Problem::internal(&MissingLink)))
}

/// The subscription the arguments ask for: the types first, then the cursor of the carrier that
/// wins, the header over the argument (FR-11, FR-13).
///
/// # Errors
///
/// The 422 problem of the first faulty argument, with one violation.
fn request(
    types: &[String],
    link: &StreamLink,
    last_event_id: Option<&str>,
) -> Result<SubscriptionRequest, Problem> {
    let types = parse_names(types.iter().map(String::as_str)).map_err(types_problem)?;
    let cursor =
        read_cursor(link.last_event_id(), last_event_id).map_err(CursorFault::violation)?;
    Ok(SubscriptionRequest { types, cursor })
}

/// Registers the hub subscriber and reports the outcome through the link. A refusal is not
/// logged here: the transport answers it with the 503 problem, which logs it once (FR-16).
///
/// # Errors
///
/// The error of the hub's refusal, with the hub's text.
fn open(
    state: &EventStreamState,
    link: &StreamLink,
    request: SubscriptionRequest,
) -> async_graphql::Result<Subscription> {
    match state.subscriptions.open(request) {
        Ok(subscription) => {
            link.opened();
            Ok(subscription)
        }
        Err(error) => {
            link.refused(error);
            Err(async_graphql::Error::new(error.to_string()))
        }
    }
}

/// The subscription and the link that its stream owns; dropping the stream drops the subscriber
/// and frees its slot (FR-19 (a)).
type Live = (Subscription, StreamLink);

/// The events of the subscription, once the transport released the link (FR-9).
fn events(subscription: Subscription, link: StreamLink) -> impl Stream<Item = EventObject> {
    stream::once(async move {
        link.released().await;
        stream::unfold((subscription, link), next_event)
    })
    .flatten()
}

/// The next event, its id handed to the link first; at the end, `replay failed` when a replay
/// could not read Kafka (FR-19 (c)), and nothing when the hub closed.
async fn next_event((mut subscription, link): Live) -> Option<(EventObject, Live)> {
    if let Some(frame) = subscription.next().await {
        link.delivered(frame.sequence);
        return Some((EventObject::from(&*frame), (subscription, link)));
    }
    if let Some(error) = subscription.failure() {
        log_replay_failed(TRANSPORT, error);
    }
    None
}

/// Subscription root of the event stream.
#[derive(Debug, Default)]
pub struct EventStreamSubscription;

#[async_graphql::Subscription]
impl EventStreamSubscription {
    /// Streams every event of the listed types (1 to 100 names); with lastEventId or the Last-Event-ID header, every retained event after that id comes first.
    async fn events(
        &self,
        ctx: &Context<'_>,
        types: Vec<String>,
        last_event_id: Option<String>,
    ) -> async_graphql::Result<impl Stream<Item = EventObject>> {
        let state = state(ctx)?;
        let link = link(ctx)?;
        let request = request(&types, link, last_event_id.as_deref()).map_err(problem_to_error)?;
        let subscription = open(state, link, request)?;
        // The `Subscription` macro accepts only async fields; the stream is ready at once.
        std::future::ready(Ok(events(subscription, link.clone()))).await
    }
}

#[cfg(test)]
mod tests;
