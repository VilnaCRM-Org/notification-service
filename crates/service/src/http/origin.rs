//! The one allow-list of browser origins (FR-28): the CORS layer of the HTTP routes and the origin
//! check of a WebSocket handshake, which a browser does not subject to CORS.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::header::{CONTENT_TYPE, ORIGIN, SEC_WEBSOCKET_KEY};
use axum::http::{HeaderName, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};
use shared_application::http::problem::Problem;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::config::AllowedOrigins;

/// How long a browser may cache a preflight answer.
pub const PREFLIGHT_MAX_AGE: Duration = Duration::from_secs(600);

/// The `detail` of the 403 problem of a refused handshake.
pub const ORIGIN_NOT_ALLOWED: &str = "the origin is not allowed";

/// The CORS layer of the allowed origins, or `None` when no origin is allowed: the router then has
/// no CORS layer at all, so no existing answer changes.
#[must_use]
pub fn cors(origins: &AllowedOrigins) -> Option<CorsLayer> {
    let allow_origin = match origins {
        AllowedOrigins::NoOrigin => return None,
        AllowedOrigins::Any => AllowOrigin::any(),
        AllowedOrigins::List(list) => AllowOrigin::list(list.iter().cloned()),
    };
    Some(
        CorsLayer::new()
            .allow_origin(allow_origin)
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([CONTENT_TYPE, HeaderName::from_static("last-event-id")])
            .max_age(PREFLIGHT_MAX_AGE),
    )
}

/// Answers a WebSocket handshake (a request with `sec-websocket-key`) whose `origin` is present and
/// not allowed with the 403 problem; passes every other request on.
pub(crate) async fn websocket_origin(
    State(origins): State<Arc<AllowedOrigins>>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let refused = headers.contains_key(SEC_WEBSOCKET_KEY)
        && headers
            .get(ORIGIN)
            .is_some_and(|origin| !origins.allows(origin));
    if refused {
        Problem::new(StatusCode::FORBIDDEN)
            .with_detail(ORIGIN_NOT_ALLOWED)
            .into_response()
    } else {
        next.run(request).await
    }
}

#[cfg(test)]
mod tests;
