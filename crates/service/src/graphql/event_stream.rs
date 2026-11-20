//! The event stream of the `GraphQL` path: graphql-sse's distinct connections mode (FR-6 to FR-9).
//!
//! The operation is driven before the head, inside the request budget, until the `events` field
//! registered its hub subscriber, the hub refused it, or the operation produced its first result
//! (FR-9). The decision is read from the phase of the [`StreamLink`], which the field sets, never
//! from which future completed first. A refusal is the 503 problem; otherwise the head is written,
//! then a comment, then the results: one `next` and `complete` for a single result, one `next`
//! with the event id per event for a subscription, which never writes `complete` (FR-7, FR-8,
//! FR-19). A subscription body also writes the comment whenever `STREAM_KEEP_ALIVE` passes without
//! a block, the interval of the other streaming routes, read from the `EventStreamState` in the
//! schema data (FR-17). The blocks are written as bytes, because `axum::response::sse` cannot write
//! the bare `data:` line of `complete`.

use std::borrow::Cow;
use std::convert::Infallible;
use std::pin::Pin;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, VARY};
use axum::http::{HeaderName, HeaderValue};
use axum::response::{IntoResponse as _, Response};
use event_stream_application::graphql::{StreamLink, StreamPhase};
use event_stream_application::http::EventStreamState;
use event_stream_domain::Sequence;
use futures_util::future;
use futures_util::stream::{self, BoxStream, Stream, StreamExt as _};
use shared_application::http::Problem;
use tokio::time::{Sleep, sleep};

use crate::graphql::schema::ApiSchema;

/// The comment block: the first block of every event stream (FR-17).
const KEEP_ALIVE: &str = ":\n\n";

/// The block that follows a single result, with the bare `data:` line of the protocol.
const COMPLETE: &str = "event: complete\ndata:\n\n";

/// The header that keeps a proxy from holding blocks back, as on the SSE route.
const X_ACCEL_BUFFERING: HeaderName = HeaderName::from_static("x-accel-buffering");

/// The result written if a result could not be encoded, which `serde_json` never refuses for an
/// execution result.
const UNENCODABLE: &str = r#"{"data":null,"errors":[{"message":"Internal server error"}]}"#;

/// The execution results of one operation.
type Responses = BoxStream<'static, async_graphql::Response>;

/// The schema was built without the `EventStreamState` data, which holds the keep-alive interval
/// of the event stream: a wiring fault that only a test can build.
#[derive(Debug, thiserror::Error)]
#[error("the GraphQL schema has no EventStreamState data for the event stream")]
struct MissingStreamState;

/// Answers one request that negotiated an event stream and passed the transport checks: the event
/// stream, the 503 problem of a subscriber the hub refused (FR-16), which logs
/// `subscription rejected` once, or the 500 problem of a schema without `EventStreamState`, which
/// logs its error chain once before anything runs.
///
/// The link carries `last_event_id` to the field and reports its phase back. Dropping this future
/// before it answers, as the request budget does, drops the operation and with it any subscriber
/// the field registered (FR-9). The budget bounds only this future: the body of a subscription
/// outlives it.
pub(crate) async fn respond(
    schema: &ApiSchema,
    request: async_graphql::Request,
    last_event_id: Option<HeaderValue>,
) -> Response {
    let Some(state) = schema.data::<EventStreamState>() else {
        return Problem::internal(&MissingStreamState).into_response();
    };
    let keep_alive = state.settings.keep_alive;
    let link = StreamLink::new(last_event_id);
    let mut responses = schema.execute_stream(request.data(link.clone()));
    let first = tokio::select! {
        biased;
        () = link.decided() => None,
        result = responses.next() => result,
    };
    match link.phase() {
        StreamPhase::Refused(error) => Problem::from(error).into_response(),
        StreamPhase::Pending => stream_answer(single_result(first)),
        StreamPhase::Opened | StreamPhase::Released => {
            link.release();
            stream_answer(subscription(responses, link, keep_alive))
        }
    }
}

/// The body of a query, an error result or an operation that ran nothing: the comment, the one
/// result if there is one, then `complete`. Every block is known before the head.
fn single_result(result: Option<async_graphql::Response>) -> impl Stream<Item = Bytes> {
    let next = result.map(|result| Bytes::from(next_block(&result, None)));
    let blocks = [
        Some(Bytes::from(KEEP_ALIVE)),
        next,
        Some(Bytes::from(COMPLETE)),
    ];
    stream::iter(blocks.into_iter().flatten())
}

/// The body of a subscription: the comment, then one `next` block per result with the event id
/// the field handed to the link, and the comment whenever `keep_alive` passes without a block, also
/// while a replay waits (FR-17, NFR-2). It ends with the results, without `complete`.
fn subscription(
    responses: Responses,
    link: StreamLink,
    keep_alive: Duration,
) -> impl Stream<Item = Bytes> {
    let body = SubscriptionBody {
        responses,
        link,
        keep_alive,
        timer: Box::pin(sleep(keep_alive)),
    };
    stream::once(future::ready(Bytes::from(KEEP_ALIVE))).chain(stream::unfold(body, next_chunk))
}

/// What a subscription body keeps between two chunks.
struct SubscriptionBody {
    /// The execution results of the subscription.
    responses: Responses,
    /// The link that hands over the event id of each result.
    link: StreamLink,
    /// The longest time without a block (`STREAM_KEEP_ALIVE`).
    keep_alive: Duration,
    /// Expires `keep_alive` after the last chunk.
    timer: Pin<Box<Sleep>>,
}

/// The next chunk of a subscription body: the `next` block of the next result, or the comment when
/// the timer expires first; `None` once the results end. Every chunk restarts the timer, in place,
/// with the deadline `keep_alive` from now (tokio time, which tests can pause; `sleep` adds it
/// without overflow).
///
/// The results are polled first, so a result that is ready is never held back by the comment.
/// Dropping the `next` future when the timer wins loses nothing: the stream keeps its state.
async fn next_chunk(mut body: SubscriptionBody) -> Option<(Bytes, SubscriptionBody)> {
    let chunk = tokio::select! {
        biased;
        result = body.responses.next() => {
            Bytes::from(next_block(&result?, body.link.take_delivered()))
        }
        () = &mut body.timer => Bytes::from(KEEP_ALIVE),
    };
    body.timer.set(sleep(body.keep_alive));
    Some((chunk, body))
}

/// The head of every event stream (FR-7) over the blocks of `body`.
fn stream_answer(body: impl Stream<Item = Bytes> + Send + 'static) -> Response {
    (
        [
            (CONTENT_TYPE, HeaderValue::from_static("text/event-stream")),
            (CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            (X_ACCEL_BUFFERING, HeaderValue::from_static("no")),
            (VARY, HeaderValue::from_static("accept")),
        ],
        Body::from_stream(body.map(Ok::<_, Infallible>)),
    )
        .into_response()
}

/// The `next` block of `result`: `event: next`, the `id:` line of a subscription result, then the
/// result as compact JSON on one `data:` line, every line break inside a string escaped.
fn next_block(result: &async_graphql::Response, id: Option<Sequence>) -> String {
    let data = serde_json::to_string(result).map_or(Cow::Borrowed(UNENCODABLE), Cow::Owned);
    id.map_or_else(
        || format!("event: next\ndata: {data}\n\n"),
        |id| format!("event: next\nid: {id}\ndata: {data}\n\n"),
    )
}

#[cfg(test)]
mod tests;
