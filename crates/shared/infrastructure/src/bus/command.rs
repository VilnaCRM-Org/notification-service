//! In-process command bus.

use std::any::TypeId;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::marker::PhantomData;
use std::sync::Arc;

use shared_domain::{Command, CommandBus, CommandHandler, ErasedBusError, ErasedMessage};

use super::DuplicateHandler;

#[async_trait::async_trait]
trait ErasedCommandHandler: Send + Sync {
    async fn handle(&self, command: ErasedMessage) -> Result<(), ErasedBusError>;
}

struct CommandHandlerAdapter<C, H> {
    handler: H,
    command: PhantomData<fn() -> C>,
}

impl<C: Command, H: CommandHandler<C>> CommandHandlerAdapter<C, H> {
    async fn run(&self, command: ErasedMessage) -> Result<(), ErasedBusError> {
        let command = command
            .downcast::<C>()
            .map_err(|_other| ErasedBusError::NotRegistered { message: C::NAME })?;
        self.handler
            .handle(*command)
            .await
            .map_err(|error| ErasedBusError::Handler(Box::new(error)))
    }
}

#[async_trait::async_trait]
impl<C: Command, H: CommandHandler<C>> ErasedCommandHandler for CommandHandlerAdapter<C, H> {
    async fn handle(&self, command: ErasedMessage) -> Result<(), ErasedBusError> {
        self.run(command).await
    }
}

/// Command bus that dispatches to handlers registered at start-up.
#[derive(Default)]
pub struct InMemoryCommandBus {
    handlers: HashMap<TypeId, Arc<dyn ErasedCommandHandler>>,
}

impl InMemoryCommandBus {
    /// An empty bus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the handler of command type `C`.
    ///
    /// # Errors
    ///
    /// Returns `DuplicateHandler` when `C` already has a handler.
    pub fn register<C: Command, H: CommandHandler<C>>(
        mut self,
        handler: H,
    ) -> Result<Self, DuplicateHandler> {
        match self.handlers.entry(TypeId::of::<C>()) {
            Entry::Occupied(_) => Err(DuplicateHandler { message: C::NAME }),
            Entry::Vacant(slot) => {
                slot.insert(Arc::new(CommandHandlerAdapter {
                    handler,
                    command: PhantomData,
                }));
                Ok(self)
            }
        }
    }

    async fn dispatch(
        &self,
        command_type: TypeId,
        name: &'static str,
        command: ErasedMessage,
    ) -> Result<(), ErasedBusError> {
        let handler = self
            .handlers
            .get(&command_type)
            .ok_or(ErasedBusError::NotRegistered { message: name })?;
        handler.handle(command).await
    }
}

impl std::fmt::Debug for InMemoryCommandBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryCommandBus")
            .field("handlers", &self.handlers.len())
            .finish()
    }
}

#[async_trait::async_trait]
impl CommandBus for InMemoryCommandBus {
    async fn dispatch_erased(
        &self,
        command_type: TypeId,
        name: &'static str,
        command: ErasedMessage,
    ) -> Result<(), ErasedBusError> {
        self.dispatch(command_type, name, command).await
    }
}

#[cfg(test)]
mod tests;
