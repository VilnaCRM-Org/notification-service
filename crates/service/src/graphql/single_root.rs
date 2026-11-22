//! The one-root-field rule of a subscription sent over an event stream (FR-14).
//!
//! async-graphql makes one stream for every field selection written directly at the root of a
//! subscription, skips a fragment at the root, and has no validation rule that bounds them. The
//! event-stream transport therefore checks the direct-field rule before it runs the operation, so
//! that one request registers exactly one hub subscriber: the root selection set of the
//! subscription that runs holds exactly one selection, which is a field (no fragment spread or
//! inline fragment), whose name does not start with `__` and which carries no `@skip` or
//! `@include`. The same response name written twice counts as two selections. Fragments and
//! directives inside the selection set of that field are not the rule's concern.
//!
//! The rule runs on the parsed document before async-graphql's validation, so a document that
//! breaks both is answered with the rule's error (FR-6 step 3). The JSON transport never runs it
//! (FR-5).

use async_graphql::parser::types::{
    DocumentOperations, ExecutableDocument, OperationDefinition, OperationType, Selection,
};
use async_graphql::{Pos, Positioned, ServerError};
use shared_application::graphql::problem_to_error;
use shared_application::http::{Problem, Violation};

/// The property path of the violation: the document of the request.
const PROPERTY_PATH: &str = "query";

/// The stable code of the violation.
const CODE: &str = "single_root_field";

/// The message of the violation, which holds no value of the request (NFR-8).
const MESSAGE: &str = "a subscription operation must select exactly one root field";

/// The one-root-field rule (FR-14) for the operation that runs. `Ok` when the document does not
/// parse, names no runnable operation, or selects a query or mutation: async-graphql answers those.
///
/// The document is parsed once and cached in `request`, which the execution then reuses.
///
/// # Errors
///
/// The `VALIDATION_FAILED` error of a subscription that breaks the rule, located at the selection
/// that breaks it: the second root selection, the root fragment, the introspection field or the
/// field with the directive.
pub(crate) fn check(request: &mut async_graphql::Request) -> Result<(), ServerError> {
    let operation_name = request.operation_name.clone();
    let Ok(document) = request.parsed_query() else {
        return Ok(());
    };
    let Some(operation) = runnable(document, operation_name.as_deref()) else {
        return Ok(());
    };
    if operation.ty != OperationType::Subscription {
        return Ok(());
    }
    breach(&operation.selection_set.node.items).map_or(Ok(()), |pos| Err(violation(pos)))
}

/// The operation that runs, chosen as async-graphql's `prepare_request` chooses it: the operation
/// that `operation_name` names among named operations, or the only operation when no name is
/// given. `None` where async-graphql reports an error instead: a name for an anonymous operation,
/// an unknown name, or several operations without a name.
fn runnable<'document>(
    document: &'document ExecutableDocument,
    operation_name: Option<&str>,
) -> Option<&'document OperationDefinition> {
    match (&document.operations, operation_name) {
        (DocumentOperations::Single(operation), None) => Some(&operation.node),
        (DocumentOperations::Single(_), Some(_)) => None,
        (DocumentOperations::Multiple(operations), Some(name)) => {
            operations.get(name).map(|operation| &operation.node)
        }
        (DocumentOperations::Multiple(operations), None) => operations
            .values()
            .next()
            .filter(|_| operations.len() == 1)
            .map(|operation| &operation.node),
    }
}

/// The position of the selection that breaks the rule in the root selection set of a
/// subscription: the second selection when there are several, else the one selection when it is
/// not a plain field. `None` when the set holds one plain field, or nothing, which the parser never
/// yields.
fn breach(selections: &[Positioned<Selection>]) -> Option<Pos> {
    match selections {
        [selection] => breaks_rule(&selection.node).then_some(selection.pos),
        [_, second, ..] => Some(second.pos),
        [] => None,
    }
}

/// True for a fragment, an introspection field and a field under `@skip` or `@include`.
fn breaks_rule(selection: &Selection) -> bool {
    match selection {
        Selection::Field(field) => {
            field.node.name.node.starts_with("__")
                || field.node.directives.iter().any(|directive| {
                    matches!(directive.node.name.node.as_str(), "skip" | "include")
                })
        }
        Selection::FragmentSpread(_) | Selection::InlineFragment(_) => true,
    }
}

/// The `VALIDATION_FAILED` error of a breach at `pos`, with the one violation of the rule.
fn violation(pos: Pos) -> ServerError {
    problem_to_error(Problem::validation_failed(vec![Violation {
        property_path: PROPERTY_PATH.to_owned(),
        message: MESSAGE.to_owned(),
        code: CODE.to_owned(),
    }]))
    .into_server_error(pos)
}

#[cfg(test)]
mod tests;
