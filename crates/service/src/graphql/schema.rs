//! The merged GraphQL schema: one root per operation type, one field per bounded context.
#![expect(
    clippy::useless_let_if_seq,
    reason = "the MergedObject derive expands to code that triggers this lint"
)]

use async_graphql::{
    EmptyMutation, MergedObject, MergedSubscription, SDLExportOptions, Schema, SchemaBuilder,
};
use event_stream_application::graphql::{EventStreamQuery, EventStreamSubscription};

/// Query root.
#[derive(Debug, MergedObject, Default)]
pub struct Query(EventStreamQuery);

/// Subscription root.
#[derive(Debug, MergedSubscription, Default)]
pub struct Subscription(EventStreamSubscription);

/// The schema type of the service: no mutation root.
pub type ApiSchema = Schema<Query, EmptyMutation, Subscription>;

/// The deepest field nesting a document may select; the standard introspection query of the
/// explorer still fits.
pub const MAX_DEPTH: usize = 16;

/// The highest complexity a document may reach: every selected field counts one, except
/// `Event.data`, which counts 501, so a document selects it at most once (FR-1).
pub const MAX_COMPLEXITY: usize = 1000;

/// A builder for the schema, bounded in depth and complexity; the caller adds the state of every
/// context with `.data(..)`.
#[must_use]
pub fn builder() -> SchemaBuilder<Query, EmptyMutation, Subscription> {
    Schema::build(Query::default(), EmptyMutation, Subscription::default())
        .limit_depth(MAX_DEPTH)
        .limit_complexity(MAX_COMPLEXITY)
}

/// The schema definition language of the schema, sorted so that it is byte-stable (AD-17).
#[must_use]
pub fn sdl() -> String {
    builder().finish().sdl_with_options(
        SDLExportOptions::new()
            .sorted_fields()
            .sorted_arguments()
            .sorted_enum_items()
            .prefer_single_line_descriptions()
            .use_space_ident()
            .indent_width(2),
    )
}

#[cfg(test)]
mod tests;
