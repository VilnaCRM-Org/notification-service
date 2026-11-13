//! The `application/problem+json` document and its conversions (AD-06).

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use shared_domain::{BusError, error_chain};
use utoipa::ToSchema;
use validator::{ValidationErrors, ValidationErrorsKind};

/// Media type of every error response.
pub const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";
/// `type` of a problem whose status says everything.
pub const TYPE_BLANK: &str = "about:blank";
/// `type` of a constraint violation in a body or a query.
pub const TYPE_VALIDATION_FAILED: &str = "/problems/validation-failed";
/// `type` of a failed health check.
pub const TYPE_DEPENDENCY_UNAVAILABLE: &str = "/problems/dependency-unavailable";

const VALIDATION_DETAIL: &str = "The request failed validation.";

/// One violated constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Violation {
    /// Wire name of the invalid field.
    pub property_path: String,
    /// Human-readable explanation.
    pub message: String,
    /// Stable code of the violated constraint.
    pub code: String,
}

/// One dependency that did not answer its probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct DependencyFailure {
    /// `database`, `cache` or `message_broker`.
    pub dependency: String,
    /// Failure message of the probe.
    pub message: String,
}

/// An RFC 9457 problem document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Problem {
    /// `about:blank` or a relative URI reference.
    #[schema(value_type = String)]
    pub r#type: &'static str,
    /// Canonical reason phrase of the status.
    #[schema(value_type = String)]
    pub title: &'static str,
    /// HTTP status code.
    #[schema(minimum = 100, maximum = 599)]
    pub status: u16,
    /// Explanation specific to this occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Violated constraints, for `/problems/validation-failed`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<Violation>,
    /// Failed probes, for `/problems/dependency-unavailable`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<DependencyFailure>,
}

impl Problem {
    /// A problem of type `about:blank` with the canonical title of `status` and no detail.
    #[must_use]
    pub fn new(status: StatusCode) -> Self {
        Self {
            r#type: TYPE_BLANK,
            title: status.canonical_reason().unwrap_or("Unknown"),
            status: status.as_u16(),
            detail: None,
            violations: Vec::new(),
            failures: Vec::new(),
        }
    }

    /// The same problem with `detail` set.
    #[must_use]
    pub fn with_detail<D: Into<String>>(self, detail: D) -> Self {
        Self {
            detail: Some(detail.into()),
            ..self
        }
    }

    /// The 422 problem that lists `violations` sorted by property path, then by code.
    #[must_use]
    pub fn validation_failed(mut violations: Vec<Violation>) -> Self {
        violations.sort_by(|left, right| {
            left.property_path
                .cmp(&right.property_path)
                .then_with(|| left.code.cmp(&right.code))
        });
        Self {
            r#type: TYPE_VALIDATION_FAILED,
            violations,
            ..Self::new(StatusCode::UNPROCESSABLE_ENTITY).with_detail(VALIDATION_DETAIL)
        }
    }

    /// The 500 problem that lists the failed probes; the detail joins their messages with `; `.
    #[must_use]
    pub fn dependency_unavailable(failures: Vec<DependencyFailure>) -> Self {
        let detail = failures
            .iter()
            .map(|failure| failure.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        Self {
            r#type: TYPE_DEPENDENCY_UNAVAILABLE,
            failures,
            ..Self::new(StatusCode::INTERNAL_SERVER_ERROR).with_detail(detail)
        }
    }

    /// The 500 problem without detail; the error chain of `cause` is logged once at `ERROR`.
    #[must_use]
    pub fn internal(cause: &(dyn std::error::Error + 'static)) -> Self {
        log_request_failed(&error_chain(cause));
        Self::new(StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// The problem of an axum rejection: its own status and text, or the internal problem when the
    /// rejection reports a server error.
    fn rejection(
        status: StatusCode,
        text: String,
        cause: &(dyn std::error::Error + 'static),
    ) -> Self {
        if status.is_server_error() {
            Self::internal(cause)
        } else {
            Self::new(status).with_detail(text)
        }
    }
}

fn log_request_failed(error: &str) {
    tracing::error!(error = %error, "request failed");
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = (status, Json(self)).into_response();
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(PROBLEM_CONTENT_TYPE));
        response
    }
}

impl From<ValidationErrors> for Problem {
    fn from(errors: ValidationErrors) -> Self {
        let mut violations = Vec::new();
        collect_violations("", &errors, &mut violations);
        Self::validation_failed(violations)
    }
}

#[expect(
    clippy::iter_over_hash_type,
    reason = "the violations are sorted by `Problem::validation_failed`"
)]
fn collect_violations(prefix: &str, errors: &ValidationErrors, out: &mut Vec<Violation>) {
    for (field, kind) in errors.errors() {
        let path = format!("{prefix}{}", camel_case(field));
        match kind {
            ValidationErrorsKind::Field(failed) => out.extend(failed.iter().map(|error| {
                Violation {
                    property_path: path.clone(),
                    message: error
                        .message
                        .as_ref()
                        .map_or_else(|| error.code.to_string(), ToString::to_string),
                    code: error.code.to_string(),
                }
            })),
            ValidationErrorsKind::Struct(nested) => {
                collect_violations(&format!("{path}."), nested, out);
            }
            ValidationErrorsKind::List(items) => {
                for (index, nested) in items {
                    collect_violations(&format!("{path}[{index}]."), nested, out);
                }
            }
        }
    }
}

fn camel_case(name: &str) -> String {
    let mut camel = String::with_capacity(name.len());
    let mut upper = false;
    for character in name.chars() {
        if character == '_' {
            upper = true;
        } else if upper {
            camel.extend(character.to_uppercase());
            upper = false;
        } else {
            camel.push(character);
        }
    }
    camel
}

impl From<JsonRejection> for Problem {
    fn from(rejection: JsonRejection) -> Self {
        Self::rejection(rejection.status(), rejection.body_text(), &rejection)
    }
}

impl From<PathRejection> for Problem {
    fn from(rejection: PathRejection) -> Self {
        Self::rejection(rejection.status(), rejection.body_text(), &rejection)
    }
}

impl From<QueryRejection> for Problem {
    fn from(rejection: QueryRejection) -> Self {
        Self::rejection(rejection.status(), rejection.body_text(), &rejection)
    }
}

impl<E: std::error::Error + Into<Self> + 'static> From<BusError<E>> for Problem {
    fn from(error: BusError<E>) -> Self {
        match error {
            BusError::Handler(inner) => inner.into(),
            BusError::NotRegistered { .. } => Self::internal(&error),
        }
    }
}

#[cfg(test)]
mod tests;
