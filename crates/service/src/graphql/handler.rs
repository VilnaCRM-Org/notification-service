//! The GraphQL transport: `POST /api/graphql` and the development explorer (AD-18).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::CONTENT_SECURITY_POLICY;
use axum::response::{Html, IntoResponse as _, Response};
use serde_json::{Map, Value};
use shared_application::http::{ApiJson, Problem};

use crate::graphql::schema::ApiSchema;

/// Policy of the explorer page: the one third-party origin it loads from, and nothing else.
pub const EXPLORER_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src https://unpkg.com 'unsafe-inline'; style-src https://unpkg.com 'unsafe-inline'; img-src data:; font-src data:; connect-src 'self'; frame-ancestors 'none'";

const EXPLORER_PAGE: &str = include_str!("explorer.html");

/// Executes one GraphQL operation.
///
/// The body must be a JSON object with a string member `query`; operation errors are part of the
/// 200 response.
///
/// # Errors
///
/// The 422 problem when the body is not a GraphQL request. A body that is not JSON, has the wrong
/// media type or is too large is rejected by the extractor with its own problem.
pub async fn execute(
    State(schema): State<ApiSchema>,
    ApiJson(body): ApiJson<Map<String, Value>>,
) -> Result<Json<async_graphql::Response>, Problem> {
    let request = to_request(body).ok_or_else(|| {
        Problem::new(StatusCode::UNPROCESSABLE_ENTITY)
            .with_detail("The request body is not a GraphQL request.")
    })?;
    Ok(Json(schema.execute(request).await))
}

/// The request for a body that has a string member `query` and otherwise converts.
fn to_request(body: Map<String, Value>) -> Option<async_graphql::Request> {
    if !body.get("query").is_some_and(Value::is_string) {
        return None;
    }
    serde_json::from_value(Value::Object(body)).ok()
}

/// The development explorer page with its own content security policy.
pub async fn explorer() -> Response {
    (
        [(CONTENT_SECURITY_POLICY, EXPLORER_CONTENT_SECURITY_POLICY)],
        Html(EXPLORER_PAGE),
    )
        .into_response()
}

#[cfg(test)]
mod tests;
