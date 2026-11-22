//! The documentation page, the `OpenAPI` document as JSON and the Swagger UI assets (AD-19), each
//! an operation of the document it serves (ADR 0022).

use std::sync::Arc;

use axum::Json;
use axum::http::StatusCode;
use axum::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE};
use axum::response::{Html, IntoResponse as _, Response};
use shared_application::http::{ApiPath, Problem};
use utoipa::openapi::schema::{ObjectBuilder, Schema, Type};
use utoipa::openapi::{OpenApi, RefOr};

use crate::openapi;

/// Policy of the documentation page: everything from the service itself, nothing third-party.
pub const DOCS_CONTENT_SECURITY_POLICY: &str = "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'";

/// Where the Swagger UI start-up script finds the document.
const DOCUMENT_URL: &str = "/api/docs/openapi.json";

const DOCS_PAGE: &str = include_str!("docs.html");

/// The asset of the examples: the style sheet the page loads first.
const EXAMPLE_ASSET: &str = "swagger-ui.css";

/// The description of `GET /api/docs/{file}`.
pub(crate) const ASSET_TEXT: &str = "Serves one file of the embedded Swagger UI, which the page \
    loads: the style sheets, the scripts, the HTML pages and the icons of its distribution, each \
    with its media type. Every file is one path segment deep; a deeper path matches no route.";

/// The description of the 400 problem of `GET /api/docs/{file}`.
const ASSET_BAD_REQUEST_TEXT: &str =
    "The file name is not percent-encoded UTF-8; the detail names the `file` parameter.";

/// The media type of the icons of the page, the one binary body of `GET /api/docs/{file}`.
const PNG: &str = "image/png";

/// The bytes of an icon of the page, the `image/png` body of `GET /api/docs/{file}`.
///
/// Its schema is the binary schema of `OpenAPI` 3.1, a string of that content type, with no
/// example, since no inline text can show the bytes of an image (FR-26). A documentation type:
/// the handler answers the bytes of the embedded file.
#[derive(Debug)]
pub struct PngBytes;

impl utoipa::PartialSchema for PngBytes {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::String)
            .content_media_type(PNG)
            .into()
    }
}

impl utoipa::ToSchema for PngBytes {}

/// `GET /api/docs`: the page, answered directly with no redirect.
#[utoipa::path(
    get,
    path = "/api/docs",
    operation_id = "api_docs_get",
    tag = "Documentation",
    summary = "Serves the API documentation page.",
    description = "Serves the Swagger UI page, which renders `GET /api/docs/openapi.json` and loads its files from `GET /api/docs/{file}`, nothing from a third-party origin. It is answered directly, without a redirect, under its own content security policy.",
    responses(
        (status = 200, description = "The documentation page.", body = String, content_type = "text/html",
            headers(("content-security-policy" = String, description = "The policy of the page: everything from the service itself, nothing third-party.")),
            example = json!("<!doctype html>")),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json",
            example = json!(Problem::new(StatusCode::INTERNAL_SERVER_ERROR))),
    )
)]
pub async fn docs_page() -> Response {
    (
        [(CONTENT_SECURITY_POLICY, DOCS_CONTENT_SECURITY_POLICY)],
        Html(DOCS_PAGE),
    )
        .into_response()
}

/// `GET /api/docs/openapi.json`: the `OpenAPI` document.
#[utoipa::path(
    get,
    path = "/api/docs/openapi.json",
    operation_id = "api_docs_openapi_json_get",
    tag = "Documentation",
    summary = "Serves the OpenAPI document.",
    description = "Serves this document as JSON: every operation the service serves, the one `make generate-openapi-spec` writes to `.github/openapi-spec/spec.yaml`.",
    responses(
        (status = 200, description = "The OpenAPI document.", body = BTreeMap<String, Value>, content_type = "application/json",
            example = json!({"openapi": "3.1.0"})),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json",
            example = json!(Problem::new(StatusCode::INTERNAL_SERVER_ERROR))),
    )
)]
pub async fn openapi_json() -> Json<OpenApi> {
    Json(openapi::document())
}

/// `GET /api/docs/{file}`: an embedded Swagger UI file, or the 404 problem.
///
/// A file name that cannot be decoded is the 400 problem of the extractor.
#[utoipa::path(
    get,
    path = "/api/docs/{file}",
    operation_id = "api_docs_file_get",
    tag = "Documentation",
    summary = "Serves a file of the documentation page.",
    description = ASSET_TEXT,
    params(("file" = String, Path, description = "The name of the file, as the page references it.", example = json!(EXAMPLE_ASSET))),
    responses(
        (status = 200, description = "The file, with its media type.", content(
            (String = "text/css", example = json!("/* the style sheet */")),
            (String = "text/javascript", example = json!("/* the script */")),
            (String = "text/html", example = json!("<!doctype html>")),
            (inline(PngBytes) = "image/png"),
        )),
        (status = 400, description = ASSET_BAD_REQUEST_TEXT, body = Problem, content_type = "application/problem+json",
            example = json!(Problem::new(StatusCode::BAD_REQUEST).with_detail("Invalid URL: Invalid UTF-8 in `file`"))),
        (status = 404, description = "No file of the page has this name.", body = Problem, content_type = "application/problem+json",
            example = json!(Problem::new(StatusCode::NOT_FOUND))),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json",
            example = json!(Problem::new(StatusCode::INTERNAL_SERVER_ERROR))),
    )
)]
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
