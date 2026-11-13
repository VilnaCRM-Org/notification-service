//! GraphQL error helper: operation errors carry a stable `extensions.code` (AD-18).

use async_graphql::indexmap::IndexMap;
use async_graphql::{Error, ErrorExtensions as _, Name, Value};

use crate::http::{Problem, Violation};

const VALIDATION_FAILED: &str = "VALIDATION_FAILED";
const INTERNAL_SERVER_ERROR: &str = "INTERNAL_SERVER_ERROR";

/// Converts a problem into a GraphQL error: `VALIDATION_FAILED` with the violations for a 422,
/// `INTERNAL_SERVER_ERROR` without detail for every other status.
#[must_use]
pub fn problem_to_error(problem: Problem) -> Error {
    if problem.status == 422 {
        validation_error(problem)
    } else {
        Error::new("Internal server error").extend_with(|_error, extensions| {
            extensions.set("code", INTERNAL_SERVER_ERROR);
        })
    }
}

fn validation_error(problem: Problem) -> Error {
    let message = problem.detail.unwrap_or_else(|| problem.title.to_owned());
    let violations = problem
        .violations
        .into_iter()
        .map(violation_value)
        .collect();
    Error::new(message).extend_with(|_error, extensions| {
        extensions.set("code", VALIDATION_FAILED);
        extensions.set("violations", Value::List(violations));
    })
}

fn violation_value(violation: Violation) -> Value {
    let members = [
        ("propertyPath", violation.property_path),
        ("message", violation.message),
        ("code", violation.code),
    ];
    Value::Object(
        members
            .into_iter()
            .map(|(name, text)| (Name::new(name), Value::String(text)))
            .collect::<IndexMap<_, _>>(),
    )
}

#[cfg(test)]
mod tests;
