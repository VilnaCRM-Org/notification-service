//! Lists `ExampleApiResource`s one page at a time.

use std::sync::Arc;

use shared_domain::{PageRequest, Query, QueryHandler};
use some_module_domain::ExampleApiResourceRepository;

use crate::error::ExampleApiResourceError;
use crate::query::ExampleApiResourceView;

/// Asks for one page of resources, newest first.
#[derive(Debug, Clone, Copy)]
pub struct ListExampleApiResources {
    /// Page number, from 1.
    pub page: u32,
    /// Page size, from 1 to 100.
    pub items_per_page: u32,
}

impl Query for ListExampleApiResources {
    type Output = Vec<ExampleApiResourceView>;
    type Error = ExampleApiResourceError;
    const NAME: &'static str = "ListExampleApiResources";
}

/// Handles [`ListExampleApiResources`].
#[derive(Debug)]
pub struct ListExampleApiResourcesHandler {
    repository: Arc<dyn ExampleApiResourceRepository>,
}

impl ListExampleApiResourcesHandler {
    /// A handler over the repository.
    #[must_use]
    pub const fn new(repository: Arc<dyn ExampleApiResourceRepository>) -> Self {
        Self { repository }
    }

    async fn list(
        &self,
        query: ListExampleApiResources,
    ) -> Result<Vec<ExampleApiResourceView>, ExampleApiResourceError> {
        let page = PageRequest::new(query.page, query.items_per_page)?;
        let found = self
            .repository
            .find_page(page)
            .await
            .map_err(ExampleApiResourceError::storage)?;
        Ok(found.iter().map(ExampleApiResourceView::from).collect())
    }
}

#[async_trait::async_trait]
impl QueryHandler<ListExampleApiResources> for ListExampleApiResourcesHandler {
    async fn handle(
        &self,
        query: ListExampleApiResources,
    ) -> Result<Vec<ExampleApiResourceView>, ExampleApiResourceError> {
        self.list(query).await
    }
}

#[cfg(test)]
mod tests;
