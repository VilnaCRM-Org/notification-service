//! Request id sanitising (AD-13).
//!
//! An inbound `x-request-id` is kept only when it matches `^[A-Za-z0-9._-]{1,128}$`; otherwise it
//! is dropped, and the request-id layers behind this middleware generate a UUID.

use axum::extract::Request;
use axum::http::HeaderName;
use axum::middleware::Next;
use axum::response::Response;

/// Name of the request id header.
pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

const MAX_LENGTH: usize = 128;

fn is_acceptable(value: &[u8]) -> bool {
    (1..=MAX_LENGTH).contains(&value.len())
        && value
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(byte))
}

/// Drops an inbound request id that does not match the allowed pattern.
///
/// A request without the header, or with an acceptable value, passes unchanged.
pub async fn sanitize_request_id(mut request: Request, next: Next) -> Response {
    let acceptable = request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .is_none_or(|value| is_acceptable(value.as_bytes()));
    if !acceptable {
        request.headers_mut().remove(&REQUEST_ID_HEADER);
    }
    next.run(request).await
}

#[cfg(test)]
mod tests;
