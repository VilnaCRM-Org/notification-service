//! Example bounded context, application layer: the command and query handlers, the event
//! subscriber, the application errors and the HTTP and GraphQL adapters of the example resource.
//!
//! The use-case modules (`command`, `query`, `subscriber`, `error`) must not import `axum`,
//! `utoipa` or `async_graphql`; only `http` and `graphql` may.

pub mod command;
pub mod error;
pub mod graphql;
pub mod http;
pub mod query;
pub mod subscriber;

pub use command::create_example_api_resource::{
    CreateExampleApiResource, CreateExampleApiResourceHandler,
};
pub use error::ExampleApiResourceError;
pub use query::ExampleApiResourceView;
pub use query::get_example_api_resource::{GetExampleApiResource, GetExampleApiResourceHandler};
pub use query::list_example_api_resources::{
    ListExampleApiResources, ListExampleApiResourcesHandler,
};
pub use subscriber::audit_example_api_resource_created::AuditExampleApiResourceCreated;
