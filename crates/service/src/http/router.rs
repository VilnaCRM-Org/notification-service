//! The assembled router: routes, fallbacks and the fixed middleware order (AD-15, AD-14).

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::get;
use shared_application::http::fallback::{method_not_allowed, not_found, panic_response};
use shared_application::http::middleware::request_timeout;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::config::{AppEnv, Config};
use crate::graphql::handler::{execute, subscribe, subscribe_or_explore};
use crate::graphql::schema::ApiSchema;
use crate::http::docs::{docs_asset, docs_page, openapi_json};
use crate::http::origin;
use crate::http::ping::ping;
use crate::http::request_id::{REQUEST_ID_HEADER, sanitize_request_id};
use crate::http::vary::vary_accept;
use crate::http::{AppState, security_headers, trace};
use crate::openapi::api_router;

/// The path of the `GraphQL` route.
pub const GRAPHQL_PATH: &str = "/api/graphql";

/// Builds the complete router.
///
/// It holds the REST routes, the `GraphQL` route, liveness, the documentation routes and the two
/// fallbacks. A GET of the `GraphQL` path serves the event stream in every mode when `Accept`
/// admits it, and otherwise the explorer in `dev` and the 405 problem elsewhere, so the `Allow` of
/// every other method lists `GET`, `HEAD` and `POST`. All of them sit under one middleware stack
/// whose first layer is the outermost: the three request id layers (sanitize, set, propagate),
/// security headers, tracing, the CORS layer of the allowed origins (absent when none is allowed),
/// `vary_accept` (`Vary: accept` on every answer to a GET or HEAD of the `GraphQL` path), the
/// origin check of a WebSocket handshake, the panic catcher, the request budget and the body
/// limit. `vary_accept` sits outside the panic catcher and the request budget, so their 500 and
/// 503 answers carry `Vary: accept` too, and inside the CORS layer, which appends its own `Vary`
/// values.
pub fn assemble(state: AppState, schema: ApiSchema, config: &Config) -> Router {
    let (rest, _document) = api_router().split_for_parts();
    let graphql = if config.app_env == AppEnv::Dev {
        get(subscribe_or_explore)
    } else {
        get(subscribe)
    }
    .post(execute);
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
        .route(GRAPHQL_PATH, graphql.with_state(schema))
        .route("/ping", get(ping))
        .route("/api/docs", get(docs_page))
        .route("/api/docs/openapi.json", get(openapi_json))
        .route("/api/docs/{*file}", get(docs_asset))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(middleware)
}

#[cfg(test)]
mod tests;
