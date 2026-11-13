//! The merged GraphQL schema: one root per operation type, one field per bounded context.
#![expect(
    clippy::useless_let_if_seq,
    reason = "the MergedObject derive expands to code that triggers this lint"
)]

use async_graphql::{EmptySubscription, MergedObject, SDLExportOptions, Schema, SchemaBuilder};
use some_module_application::graphql::{ExampleApiResourceMutation, ExampleApiResourceQuery};

/// Query root.
#[derive(Debug, MergedObject, Default)]
pub struct Query(ExampleApiResourceQuery);

/// Mutation root.
#[derive(Debug, MergedObject, Default)]
pub struct Mutation(ExampleApiResourceMutation);

/// The schema type of the service: no subscription.
pub type ApiSchema = Schema<Query, Mutation, EmptySubscription>;

/// A builder for the schema; the caller adds the state of every context with `.data(..)`.
#[must_use]
pub fn builder() -> SchemaBuilder<Query, Mutation, EmptySubscription> {
    Schema::build(Query::default(), Mutation::default(), EmptySubscription)
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
