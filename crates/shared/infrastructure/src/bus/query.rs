//! In-process query bus.

use std::any::TypeId;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::marker::PhantomData;
use std::sync::Arc;

use shared_domain::{ErasedBusError, ErasedMessage, Query, QueryBus, QueryHandler};

use super::DuplicateHandler;

#[async_trait::async_trait]
trait ErasedQueryHandler: Send + Sync {
    async fn handle(&self, query: ErasedMessage) -> Result<ErasedMessage, ErasedBusError>;
}

struct QueryHandlerAdapter<Q, H> {
    handler: H,
    query: PhantomData<fn() -> Q>,
}

impl<Q: Query, H: QueryHandler<Q>> QueryHandlerAdapter<Q, H> {
    async fn run(&self, query: ErasedMessage) -> Result<ErasedMessage, ErasedBusError> {
        let query = query
            .downcast::<Q>()
            .map_err(|_other| ErasedBusError::NotRegistered { message: Q::NAME })?;
        let output = self
            .handler
            .handle(*query)
            .await
            .map_err(|error| ErasedBusError::Handler(Box::new(error)))?;
        Ok(Box::new(output))
    }
}

#[async_trait::async_trait]
impl<Q: Query, H: QueryHandler<Q>> ErasedQueryHandler for QueryHandlerAdapter<Q, H> {
    async fn handle(&self, query: ErasedMessage) -> Result<ErasedMessage, ErasedBusError> {
        self.run(query).await
    }
}

/// Query bus that dispatches to handlers registered at start-up.
#[derive(Default)]
pub struct InMemoryQueryBus {
    handlers: HashMap<TypeId, Arc<dyn ErasedQueryHandler>>,
}

impl InMemoryQueryBus {
    /// An empty bus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the handler of query type `Q`.
    ///
    /// # Errors
    ///
    /// Returns `DuplicateHandler` when `Q` already has a handler.
    pub fn register<Q: Query, H: QueryHandler<Q>>(
        mut self,
        handler: H,
    ) -> Result<Self, DuplicateHandler> {
        match self.handlers.entry(TypeId::of::<Q>()) {
            Entry::Occupied(_) => Err(DuplicateHandler { message: Q::NAME }),
            Entry::Vacant(slot) => {
                slot.insert(Arc::new(QueryHandlerAdapter {
                    handler,
                    query: PhantomData,
                }));
                Ok(self)
            }
        }
    }

    async fn ask(
        &self,
        query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError> {
        let handler = self
            .handlers
            .get(&query_type)
            .ok_or(ErasedBusError::NotRegistered { message: name })?;
        handler.handle(query).await
    }
}

impl std::fmt::Debug for InMemoryQueryBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryQueryBus")
            .field("handlers", &self.handlers.len())
            .finish()
    }
}

#[async_trait::async_trait]
impl QueryBus for InMemoryQueryBus {
    async fn ask_erased(
        &self,
        query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError> {
        self.ask(query_type, name, query).await
    }
}

#[cfg(test)]
mod tests;
