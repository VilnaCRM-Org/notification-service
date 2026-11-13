//! Responses for requests that match no handler, and for handlers that panic.

use std::any::Any;
use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse as _, Response};

use super::problem::Problem;

/// The 404 problem of an unknown route.
#[cfg_attr(
    not(test),
    expect(clippy::unused_async, reason = "axum handlers must be async")
)]
pub async fn not_found() -> Problem {
    Problem::new(StatusCode::NOT_FOUND).with_detail("No route matches the request path.")
}

/// The 405 problem of a method the route does not support.
#[cfg_attr(
    not(test),
    expect(clippy::unused_async, reason = "axum handlers must be async")
)]
pub async fn method_not_allowed() -> Problem {
    Problem::new(StatusCode::METHOD_NOT_ALLOWED)
}

/// The 500 problem for `CatchPanicLayer::custom`; the panic message is logged, never returned.
#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "`CatchPanicLayer::custom` fixes the signature of the handler"
)]
pub fn panic_response(panic: Box<dyn Any + Send + 'static>) -> Response {
    Problem::internal(&HandlerPanic(panic_message(panic.as_ref()))).into_response()
}

fn panic_message(panic: &(dyn Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(ToString::to_string)
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "the panic payload is not text".to_owned())
}

#[derive(Debug)]
struct HandlerPanic(String);

impl fmt::Display for HandlerPanic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a request handler panicked: {}", self.0)
    }
}

impl std::error::Error for HandlerPanic {}

#[cfg(test)]
mod tests;
