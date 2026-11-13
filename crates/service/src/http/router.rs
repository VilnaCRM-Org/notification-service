//! The assembled router: routes, fallbacks and the fixed middleware order (AD-15, AD-14).

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::{get, post};
use shared_application::http::fallback::{method_not_allowed, not_found, panic_response};
use shared_application::http::middleware::request_timeout;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::config::{AppEnv, Config};
use crate::graphql::handler::{execute, explorer};
use crate::graphql::schema::ApiSchema;
use crate::http::docs::{docs_asset, docs_page, openapi_json};
use crate::http::ping::ping;
use crate::http::request_id::{REQUEST_ID_HEADER, sanitize_request_id};
use crate::http::{AppState, security_headers, trace};
use crate::openapi::api_router;

/// Builds the complete router.
///
/// It holds the REST routes, the `GraphQL` route (with the explorer in `dev` only), liveness, the
/// documentation routes and the two fallbacks, all under one middleware stack whose first layer is
/// the outermost.
pub fn assemble(state: AppState, schema: ApiSchema, config: &Config) -> Router {
    let (rest, _document) = api_router().split_for_parts();
    let mut graphql = post(execute);
    if config.app_env == AppEnv::Dev {
        graphql = graphql.get(explorer);
    }
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
        .layer(CatchPanicLayer::custom(panic_response))
        .layer(from_fn_with_state(config.request_timeout, request_timeout))
        .layer(DefaultBodyLimit::max(config.max_body_bytes));
    rest.with_state(state)
        .route("/api/graphql", graphql.with_state(schema))
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
