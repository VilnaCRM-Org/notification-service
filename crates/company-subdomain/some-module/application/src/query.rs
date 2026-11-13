//! Queries of the example context and the view they answer with.

pub mod get_example_api_resource;
pub mod list_example_api_resources;

use shared_domain::Uuid;
use some_module_domain::ExampleApiResource;

/// What a query tells about a resource; handlers return views, never aggregates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleApiResourceView {
    /// Identifier of the resource.
    pub id: Uuid,
    /// Name of the resource.
    pub name: String,
    /// Description of the resource.
    pub description: String,
}

impl From<&ExampleApiResource> for ExampleApiResourceView {
    fn from(resource: &ExampleApiResource) -> Self {
        Self {
            id: resource.id().value(),
            name: resource.name().as_str().to_owned(),
            description: resource.description().as_str().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests;
