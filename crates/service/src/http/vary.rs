//! The `Vary: accept` of the answers to a GET of the `GraphQL` path (FR-5).
//!
//! The same URL answers a GET with the explorer page, the 405 problem or an event stream,
//! depending on `Accept`, so a shared cache must key its stored answers on that header.

use axum::extract::Request;
use axum::http::header::VARY;
use axum::http::{HeaderValue, Method};
use axum::middleware::Next;
use axum::response::Response;

use crate::http::router::GRAPHQL_PATH;

/// Sets `Vary: accept` on every answer to a GET or HEAD of `/api/graphql`, also on the 503 of
/// the request budget and the 500 of the panic catcher; other requests pass unchanged (FR-5).
///
/// It uses `insert`: inside the CORS layer the only `Vary` value an answer can hold is the
/// `vary: accept` of an event-stream head, so the value is never repeated, and the CORS layer then
/// appends its own values.
pub(crate) async fn vary_accept(request: Request, next: Next) -> Response {
    let method = request.method();
    let varies =
        (method == Method::GET || method == Method::HEAD) && request.uri().path() == GRAPHQL_PATH;
    let mut response = next.run(request).await;
    if varies {
        response
            .headers_mut()
            .insert(VARY, HeaderValue::from_static("accept"));
    }
    response
}

#[cfg(test)]
mod tests;
