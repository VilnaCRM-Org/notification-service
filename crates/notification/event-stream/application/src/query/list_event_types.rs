//! Lists the subscribable event types one page at a time.

use std::sync::Arc;

use shared_domain::{PageRequest, Query, QueryHandler};

use crate::catalog::EventCatalog;
use crate::error::EventStreamError;
use crate::query::EventTypeView;

/// Asks for one page of event types, in byte order of their names.
#[derive(Debug, Clone, Copy)]
pub struct ListEventTypes {
    /// Page number, from 1.
    pub page: u32,
    /// Page size, from 1 to 100.
    pub items_per_page: u32,
}

impl Query for ListEventTypes {
    type Output = Vec<EventTypeView>;
    type Error = EventStreamError;
    const NAME: &'static str = "ListEventTypes";
}

/// Handles [`ListEventTypes`] from the catalog; it reads nothing else.
#[derive(Debug)]
pub struct ListEventTypesHandler {
    catalog: Arc<EventCatalog>,
}

impl ListEventTypesHandler {
    /// A handler over the catalog.
    #[must_use]
    pub const fn new(catalog: Arc<EventCatalog>) -> Self {
        Self { catalog }
    }

    /// Reads the page from memory; nothing is awaited.
    fn list(&self, query: ListEventTypes) -> Result<Vec<EventTypeView>, EventStreamError> {
        let request = PageRequest::new(query.page, query.items_per_page)?;
        Ok(self
            .catalog
            .page(request)
            .into_iter()
            .map(EventTypeView::from)
            .collect())
    }
}

#[async_trait::async_trait]
impl QueryHandler<ListEventTypes> for ListEventTypesHandler {
    async fn handle(&self, query: ListEventTypes) -> Result<Vec<EventTypeView>, EventStreamError> {
        self.list(query)
    }
}

#[cfg(test)]
mod tests;
