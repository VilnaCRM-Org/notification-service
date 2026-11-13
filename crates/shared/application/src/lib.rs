//! Shared kernel, application layer: the problem document, the problem-aware request extractors, the
//! fallbacks, the request-budget middleware and the GraphQL error helpers that every bounded
//! context reuses.
//!
//! Use cases stay transport-free; only the `http` and `graphql` modules may name `axum`, `utoipa`
//! or `async_graphql`.

pub mod graphql;
pub mod http;
