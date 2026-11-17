//! The event stream of the `GraphQL` path: graphql-sse's distinct connections mode (FR-6 to FR-9).
//!
//! The operation is driven before the head, inside the request budget, until the `events` field
//! registered its hub subscriber, the hub refused it, or the operation produced its first result
//! (FR-9). The decision is read from the phase of the [`StreamLink`], which the field sets, never
//! from which future completed first. A refusal is the 503 problem; otherwise the head is written,
//! then a comment, then the results: one `next` and `complete` for a single result, one `next`
//! with the event id per event for a subscription, which never writes `complete` (FR-7, FR-8,
//! FR-19). The blocks are written as bytes, because `axum::response::sse` cannot write the bare
//! `data:` line of `complete`.

use std::borrow::Cow;
use std::convert::Infallible;

use axum::body::{Body, Bytes};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, VARY};
use axum::http::{HeaderName, HeaderValue};
use axum::response::{IntoResponse as _, Response};
use event_stream_application::graphql::{StreamLink, StreamPhase};
use event_stream_domain::Sequence;
use futures_util::future;
use futures_util::stream::{self, BoxStream, Stream, StreamExt as _};
use shared_application::http::Problem;

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

/// Answers one request that negotiated an event stream and passed the transport checks: the event
/// stream, or the 503 problem of a subscriber the hub refused (FR-16), which logs
/// `subscription rejected` once.
///
/// The link carries `last_event_id` to the field and reports its phase back. Dropping this future
/// before it answers, as the request budget does, drops the operation and with it any subscriber
/// the field registered (FR-9).
pub(crate) async fn respond(
    schema: &ApiSchema,
    request: async_graphql::Request,
    last_event_id: Option<HeaderValue>,
) -> Response {
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
            stream_answer(subscription(responses, link))
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
/// the field handed to the link. It ends with the results, without `complete`.
fn subscription(responses: Responses, link: StreamLink) -> impl Stream<Item = Bytes> {
    let results =
        responses.map(move |result| Bytes::from(next_block(&result, link.take_delivered())));
    stream::once(future::ready(Bytes::from(KEEP_ALIVE))).chain(results)
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
