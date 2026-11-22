//! The event stream of the `GraphQL` path: graphql-sse's distinct connections mode (FR-6 to FR-9).
//!
//! The operation is driven before the head, inside the request budget, until the `events` field
//! registered its hub subscriber, the hub refused it, or the operation produced its first result
//! (FR-9). A subscription that breaks the one-root-field rule never runs: it is answered inside
//! the stream with the rule's validation error, before the link exists (FR-6 step 3, FR-14). The
//! decision is read from the phase of the [`StreamLink`], which the field sets, never from which
//! future completed first. A refusal is the 503 problem; otherwise the head is written,
//! then a comment, then the results: one `next` and `complete` for a single result, one `next`
//! with the event id per event for a subscription, which never writes `complete` (FR-7, FR-8,
//! FR-19). A subscription body also writes the comment whenever `STREAM_KEEP_ALIVE` passes without
//! a block, the interval of the other streaming routes, read from the `EventStreamState` in the
//! schema data (FR-17). The blocks are written as bytes, because `axum::response::sse` cannot write
//! the bare `data:` line of `complete`; the results that are ready together are written as one
//! chunk.

use std::convert::Infallible;
use std::io::Write as _;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, VARY};
use axum::http::{HeaderName, HeaderValue};
use axum::response::{IntoResponse as _, Response};
use event_stream_application::graphql::{StreamLink, StreamPhase};
use event_stream_application::http::EventStreamState;
use event_stream_application::http::keep_alive::{COMMENT, with_keep_alive};
use event_stream_domain::Sequence;
use futures_util::future::{self, FutureExt as _};
use futures_util::stream::{self, BoxStream, Fuse, Stream, StreamExt as _};
use shared_application::http::Problem;

use crate::graphql::schema::ApiSchema;
use crate::graphql::single_root;

/// The comment block: the first block of every event stream (FR-17).
const KEEP_ALIVE: &str = COMMENT;

/// The block that follows a single result, with the bare `data:` line of the protocol.
const COMPLETE: &str = "event: complete\ndata:\n\n";

/// The header that keeps a proxy from holding blocks back, as on the SSE route.
const X_ACCEL_BUFFERING: HeaderName = HeaderName::from_static("x-accel-buffering");

/// The result written if a result could not be encoded, which `serde_json` never refuses for an
/// execution result.
const UNENCODABLE: &[u8] = br#"{"data":null,"errors":[{"message":"Internal server error"}]}"#;

/// The first capacity of the buffer of a chunk, in bytes.
const BLOCK_CAPACITY: usize = 512;

/// The size up to which a chunk takes another ready result, in bytes; the block that crosses it
/// is the last of its chunk.
const MAX_CHUNK_BYTES: usize = 32_768;

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
/// A subscription that breaks the one-root-field rule is answered as a single result with the
/// rule's error, after the keep-alive read and before any resolver runs; it logs nothing (FR-14,
/// FR-22).
///
/// The link carries `last_event_id` to the field and reports its phase back. Dropping this future
/// before it answers, as the request budget does, drops the operation and with it any subscriber
/// the field registered (FR-9). The budget bounds only this future: the body of a subscription
/// outlives it.
pub(crate) async fn respond(
    schema: &ApiSchema,
    mut request: async_graphql::Request,
    last_event_id: Option<HeaderValue>,
) -> Response {
    let Some(state) = schema.data::<EventStreamState>() else {
        return Problem::internal(&MissingStreamState).into_response();
    };
    let keep_alive = state.settings.keep_alive;
    if let Err(error) = single_root::check(&mut request) {
        let result = async_graphql::Response::from_errors(vec![error]);
        return stream_answer(single_result(Some(result)));
    }
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
/// while a replay waits (FR-17, NFR-2); the keep-alive is the lazy one of the SSE route, which
/// touches no timer per chunk. It ends with the results, without `complete`.
fn subscription(
    responses: Responses,
    link: StreamLink,
    keep_alive: Duration,
) -> impl Stream<Item = Bytes> + Send {
    let results = Results {
        responses: responses.fuse(),
        link,
    };
    let blocks = stream::once(future::ready(Bytes::from(KEEP_ALIVE)))
        .chain(stream::unfold(results, next_chunk));
    with_keep_alive(blocks, keep_alive)
}

/// What a subscription body keeps between two chunks.
struct Results {
    /// The execution results of the subscription; fused, because a chunk may find their end
    /// before the next chunk asks for it.
    responses: Fuse<Responses>,
    /// The link that hands over the event id of each result.
    link: StreamLink,
}

/// The next chunk of a subscription body: the `next` block of the next result, joined by the
/// blocks of the results that are ready without awaiting while the chunk is below
/// [`MAX_CHUNK_BYTES`]; `None` once the results end. A result that is not ready is not lost when
/// its poll is dropped: the stream keeps its state.
async fn next_chunk(mut results: Results) -> Option<(Bytes, Results)> {
    let first = results.responses.next().await?;
    let mut chunk = Vec::with_capacity(BLOCK_CAPACITY);
    push_next_block(&mut chunk, &first, results.link.take_delivered());
    while chunk.len() < MAX_CHUNK_BYTES
        && let Some(Some(result)) = results.responses.next().now_or_never()
    {
        push_next_block(&mut chunk, &result, results.link.take_delivered());
    }
    Some((Bytes::from(chunk), results))
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

/// The `next` block of `result` in a buffer of its own (see [`push_next_block`]).
fn next_block(result: &async_graphql::Response, id: Option<Sequence>) -> Vec<u8> {
    let mut block = Vec::with_capacity(BLOCK_CAPACITY);
    push_next_block(&mut block, result, id);
    block
}

/// Appends the `next` block of `result`: `event: next`, the `id:` line of a subscription result,
/// then the result as compact JSON on one `data:` line, every line break inside a string escaped.
/// The result is encoded straight into the buffer; writing into a `Vec` never fails.
fn push_next_block(buffer: &mut Vec<u8>, result: &async_graphql::Response, id: Option<Sequence>) {
    buffer.extend_from_slice(b"event: next\n");
    if let Some(id) = id {
        writeln!(buffer, "id: {id}").unwrap_or_default();
    }
    buffer.extend_from_slice(b"data: ");
    let start = buffer.len();
    let unencodable = serde_json::to_writer(&mut *buffer, result).is_err();
    buffer.truncate(if unencodable { start } else { buffer.len() });
    buffer.extend_from_slice(if unencodable { UNENCODABLE } else { &[] });
    buffer.extend_from_slice(b"\n\n");
}

#[cfg(test)]
mod tests;
