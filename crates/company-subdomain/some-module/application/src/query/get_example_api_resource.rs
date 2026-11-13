//! Reads one `ExampleApiResource`.

use std::sync::Arc;

use shared_domain::{Query, QueryHandler, Uuid};
use some_module_domain::{ExampleApiResourceId, ExampleApiResourceRepository};

use crate::error::ExampleApiResourceError;
use crate::query::ExampleApiResourceView;

/// Asks for the resource with `id`.
#[derive(Debug, Clone, Copy)]
pub struct GetExampleApiResource {
    /// Identifier of the wanted resource.
    pub id: Uuid,
}

impl Query for GetExampleApiResource {
    type Output = Option<ExampleApiResourceView>;
    type Error = ExampleApiResourceError;
    const NAME: &'static str = "GetExampleApiResource";
}

/// Handles [`GetExampleApiResource`].
#[derive(Debug)]
pub struct GetExampleApiResourceHandler {
    repository: Arc<dyn ExampleApiResourceRepository>,
}

impl GetExampleApiResourceHandler {
    /// A handler over the repository.
    #[must_use]
    pub const fn new(repository: Arc<dyn ExampleApiResourceRepository>) -> Self {
        Self { repository }
    }

    async fn get(
        &self,
        query: GetExampleApiResource,
    ) -> Result<Option<ExampleApiResourceView>, ExampleApiResourceError> {
        let found = self
            .repository
            .find_by_id(ExampleApiResourceId::new(query.id))
            .await
            .map_err(ExampleApiResourceError::storage)?;
        Ok(found.as_ref().map(ExampleApiResourceView::from))
    }
}

#[async_trait::async_trait]
impl QueryHandler<GetExampleApiResource> for GetExampleApiResourceHandler {
    async fn handle(
        &self,
        query: GetExampleApiResource,
    ) -> Result<Option<ExampleApiResourceView>, ExampleApiResourceError> {
        self.get(query).await
    }
}

#[cfg(test)]
mod tests;
