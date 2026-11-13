//! The documentation page, the `OpenAPI` document as JSON and the Swagger UI assets (AD-19).

use std::sync::Arc;

use axum::Json;
use axum::http::StatusCode;
use axum::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE};
use axum::response::{Html, IntoResponse as _, Response};
use shared_application::http::{ApiPath, Problem};
use utoipa::openapi::OpenApi;

use crate::openapi;

/// Policy of the documentation page: everything from the service itself, nothing third-party.
pub const DOCS_CONTENT_SECURITY_POLICY: &str = "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'";

/// Where the Swagger UI start-up script finds the document.
const DOCUMENT_URL: &str = "/api/docs/openapi.json";

const DOCS_PAGE: &str = include_str!("docs.html");

/// `GET /api/docs`: the page, answered directly with no redirect.
pub async fn docs_page() -> Response {
    (
        [(CONTENT_SECURITY_POLICY, DOCS_CONTENT_SECURITY_POLICY)],
        Html(DOCS_PAGE),
    )
        .into_response()
}

/// `GET /api/docs/openapi.json`: the `OpenAPI` document.
pub async fn openapi_json() -> Json<OpenApi> {
    Json(openapi::document())
}

/// `GET /api/docs/{*file}`: an embedded Swagger UI file, or the 404 problem.
///
/// A sub-path that cannot be decoded is the 400 problem of the extractor.
pub async fn docs_asset(ApiPath(file): ApiPath<String>) -> Response {
    let config = Arc::new(utoipa_swagger_ui::Config::from(DOCUMENT_URL));
    match utoipa_swagger_ui::serve(&file, config) {
        Ok(Some(asset)) => (
            [(CONTENT_TYPE, asset.content_type)],
            asset.bytes.into_owned(),
        )
            .into_response(),
        Ok(None) | Err(_) => Problem::new(StatusCode::NOT_FOUND).into_response(),
    }
}

#[cfg(test)]
mod tests;
