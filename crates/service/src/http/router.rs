//! The assembled router: routes, fallbacks and the fixed middleware order (AD-15, AD-14).

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware::{from_fn, from_fn_with_state};
use shared_application::http::fallback::{method_not_allowed, not_found, panic_response};
use shared_application::http::middleware::request_timeout;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::graphql::schema::ApiSchema;
use crate::http::origin;
use crate::http::request_id::{REQUEST_ID_HEADER, sanitize_request_id};
use crate::http::vary::vary_accept;
use crate::http::{AppState, security_headers, trace};
use crate::openapi::{api_router, graphql_router, operational_router};

/// The path of the `GraphQL` route.
pub const GRAPHQL_PATH: &str = "/api/graphql";

/// Builds the complete router.
///
/// It holds the REST routes, the `GraphQL` route, liveness, the documentation routes and the two
/// fallbacks. Every route comes from one of the three routers of the `OpenAPI` document
/// ([`api_router`], [`graphql_router`] and [`operational_router`]), whose handlers
/// are registered through `routes!`; each is split from its document and merged once its state is
/// applied, so every route served is an operation of the document. A GET of the `GraphQL` path
/// serves the event stream in every mode when `Accept` admits it. Otherwise it serves the explorer
/// when the schema data's `ExplorerPage` is set, as `bootstrap::assemble_router` sets it from
/// `APP_ENV` (`dev` only), and a schema without the flag answers the 405 problem with
/// `Allow: POST`; this function does not read `APP_ENV` for that choice. The `Allow` of every
/// other method lists `GET`, `HEAD` and `POST`. All of them sit under one middleware stack
/// whose first layer is the outermost: the three request id layers (sanitize, set, propagate),
/// security headers, tracing, the CORS layer of the allowed origins (absent when none is allowed),
/// `vary_accept` (`Vary: accept` on every answer to a GET or HEAD of the `GraphQL` path), the
/// origin check of a WebSocket handshake, the panic catcher, the request budget and the body
/// limit. `vary_accept` sits outside the panic catcher and the request budget, so their 500 and
/// 503 answers carry `Vary: accept` too, and inside the CORS layer, which appends its own `Vary`
/// values.
pub fn assemble(state: AppState, schema: ApiSchema, config: &Config) -> Router {
    let (rest, _document) = api_router().split_for_parts();
    let (graphql, _document) = graphql_router().split_for_parts();
    let (operational, _document) = operational_router().split_for_parts();
    let middleware = ServiceBuilder::new()
        .layer(from_fn(sanitize_request_id))
        .layer(SetRequestIdLayer::new(REQUEST_ID_HEADER, MakeRequestUuid))
        .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER))
        .layer(security_headers::layer())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::make_span)
                .on_response(trace::on_response)
                .on_failure(()),
        )
        .option_layer(origin::cors(&config.allowed_origins))
        .layer(from_fn(vary_accept))
        .layer(from_fn_with_state(
            Arc::new(config.allowed_origins.clone()),
            origin::websocket_origin,
        ))
        .layer(CatchPanicLayer::custom(panic_response))
        .layer(from_fn_with_state(config.request_timeout, request_timeout))
        .layer(DefaultBodyLimit::max(config.max_body_bytes));
    rest.with_state(state)
        .merge(graphql.with_state(schema))
        .merge(operational)
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(middleware)
}

#[cfg(test)]
mod tests;
