//! Query bus port.

use std::any::TypeId;

use crate::bus::error::{BusError, ErasedBusError, ErasedMessage};

/// A request for data.
pub trait Query: Send + 'static {
    /// Result of the handler.
    type Output: Send + 'static;
    /// Error of the handler.
    type Error: std::error::Error + Send + Sync + 'static;
    /// The type name, used in errors and logs.
    const NAME: &'static str;
}

/// Handles one query type.
#[async_trait::async_trait]
pub trait QueryHandler<Q: Query>: Send + Sync + 'static {
    /// Handles the query.
    async fn handle(&self, query: Q) -> Result<Q::Output, Q::Error>;
}

/// Dyn-compatible query bus.
#[async_trait::async_trait]
pub trait QueryBus: std::fmt::Debug + Send + Sync {
    /// Asks a type-erased query.
    async fn ask_erased(
        &self,
        query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError>;
}

impl dyn QueryBus {
    /// Asks a query and restores the output and error types.
    ///
    /// # Errors
    ///
    /// Returns the handler's error, or `NotRegistered`.
    pub async fn ask<Q: Query>(&self, query: Q) -> Result<Q::Output, BusError<Q::Error>> {
        let output = self
            .ask_erased(TypeId::of::<Q>(), Q::NAME, Box::new(query))
            .await
            .map_err(|error| error.into_typed::<Q::Error>(Q::NAME))?;
        output
            .downcast::<Q::Output>()
            .map(|output| *output)
            .map_err(|_other| BusError::NotRegistered { message: Q::NAME })
    }
}

#[cfg(test)]
mod tests;
