//! Command bus port.

use std::any::TypeId;

use crate::bus::error::{BusError, ErasedBusError, ErasedMessage};

/// A request to change state.
pub trait Command: Send + 'static {
    /// Error of the handler.
    type Error: std::error::Error + Send + Sync + 'static;
    /// The type name, used in errors and logs.
    const NAME: &'static str;
}

/// Handles one command type.
#[async_trait::async_trait]
pub trait CommandHandler<C: Command>: Send + Sync + 'static {
    /// Handles the command.
    async fn handle(&self, command: C) -> Result<(), C::Error>;
}

/// Dyn-compatible command bus.
#[async_trait::async_trait]
pub trait CommandBus: std::fmt::Debug + Send + Sync {
    /// Dispatches a type-erased command.
    async fn dispatch_erased(
        &self,
        command_type: TypeId,
        name: &'static str,
        command: ErasedMessage,
    ) -> Result<(), ErasedBusError>;
}

impl dyn CommandBus {
    /// Dispatches a command and restores the handler's error type.
    ///
    /// # Errors
    ///
    /// Returns the handler's error, or `NotRegistered`.
    pub async fn dispatch<C: Command>(&self, command: C) -> Result<(), BusError<C::Error>> {
        self.dispatch_erased(TypeId::of::<C>(), C::NAME, Box::new(command))
            .await
            .map_err(|error| error.into_typed(C::NAME))
    }
}

#[cfg(test)]
mod tests;
