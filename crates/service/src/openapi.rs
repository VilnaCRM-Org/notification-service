//! The `OpenAPI` document of every route the service serves (AD-16, FR-26, ADR 0022).
//!
//! Every route is registered through `OpenApiRouter::routes(routes!(..))`, which adds the route to
//! the axum router and its operation to the document from the same `#[utoipa::path]` annotation.
//! Three routers carry the routes, one per state their handlers take: the REST routes of the
//! contexts, the `GraphQL` path and the operational routes (liveness and documentation).

use shared_domain::BoxError;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::config::AppEnv;
use crate::graphql::handler;
use crate::graphql::schema::ApiSchema;
use crate::http::{AppState, docs, ping};

/// Version of the API contract, not of the release.
///
/// It changes only in a pull request that changes the contract: the major number when a breaking
/// change is approved, the minor number when operations or fields are added.
pub const API_VERSION: &str = "2.1.0";

/// Static metadata of the document; title, description, contact URL and version are set in
/// [`api_router`].
#[derive(Debug, OpenApi)]
#[openapi(
    info(
        contact(name = "VilnaCRM Support", email = "support@vilnacrm.com"),
        license(name = "CC0-1.0", url = "https://creativecommons.org/publicdomain/zero/1.0/"),
    ),
    servers((url = "/", description = "Current host")),
    tags(
        (name = "Documentation", description = "The API documentation page and its document."),
        (name = "EventStream", description = "Event catalog and event stream status endpoints."),
        (name = "GraphQL", description = "GraphQL queries and subscriptions over Server-Sent Events."),
        (name = "HealthCheck", description = "Runtime health-check endpoints."),
    )
)]
pub struct ApiDoc;

/// The `OpenAPI` document could not be serialised.
#[derive(Debug, thiserror::Error)]
#[error("the OpenAPI document could not be serialised")]
pub struct ExportError(#[source] BoxError);

impl ExportError {
    fn new<E>(source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self(Box::new(source))
    }
}

/// One router for every REST operation, built without any state value; its document carries the
/// metadata of the whole document.
///
/// The routes of each context are nested under `/api`.
#[must_use]
pub fn api_router() -> OpenApiRouter<AppState> {
    let mut document = ApiDoc::openapi();
    concat!(env!("CARGO_PKG_DESCRIPTION"), " API").clone_into(&mut document.info.title);
    document.info.description = Some(format!(
        "This API provides endpoints to manage {} data within VilnaCRM.",
        env!("CARGO_PKG_DESCRIPTION")
    ));
    API_VERSION.clone_into(&mut document.info.version);
    if let Some(contact) = document.info.contact.as_mut() {
        contact.url = Some(env!("CARGO_PKG_REPOSITORY").to_owned());
    }
    OpenApiRouter::with_openapi(document)
        .nest("/api", health_check_application::router())
        .nest("/api", event_stream_application::http::router())
}

/// `/api/graphql`: on GET `subscribe`, or `subscribe_or_explore` in `dev`, and `execute` on POST.
///
/// Both GET handlers carry the same annotation, so the document does not depend on the
/// environment. GET is registered first, so the `Allow` of every other method reads
/// `GET,HEAD,POST`.
#[must_use]
pub fn graphql_router(app_env: AppEnv) -> OpenApiRouter<ApiSchema> {
    let routes = if app_env == AppEnv::Dev {
        routes!(handler::subscribe_or_explore, handler::execute)
    } else {
        routes!(handler::subscribe, handler::execute)
    };
    OpenApiRouter::new().routes(routes)
}

/// `/ping` and the three documentation routes, whose handlers take no state.
#[must_use]
pub fn operational_router() -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(ping::ping))
        .routes(routes!(docs::docs_page))
        .routes(routes!(docs::openapi_json))
        .routes(routes!(docs::docs_asset))
}

/// The document: the documents of the three routers merged, with the `prod` variant of
/// [`graphql_router`]. `GET /api/docs/openapi.json` serves it and `openapi-export` writes it.
#[must_use]
pub fn document() -> utoipa::openapi::OpenApi {
    let mut document = api_router().into_openapi();
    document.merge(graphql_router(AppEnv::Prod).into_openapi());
    document.merge(operational_router().into_openapi());
    document
}

/// The document as YAML.
///
/// # Errors
///
/// Returns [`ExportError`] when the document cannot be serialised.
pub fn yaml() -> Result<String, ExportError> {
    document().to_yaml().map_err(ExportError::new)
}

#[cfg(test)]
mod tests;
