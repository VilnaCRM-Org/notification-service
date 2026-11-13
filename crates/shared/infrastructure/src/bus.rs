//! In-process command, query and event buses.

pub mod command;
pub mod event;
pub mod query;

pub use command::InMemoryCommandBus;
pub use event::InMemoryEventBus;
pub use query::InMemoryQueryBus;

/// A second handler was registered for a message that already has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a handler is already registered for `{message}`")]
pub struct DuplicateHandler {
    /// Name of the message.
    pub message: &'static str,
}

#[cfg(test)]
mod tests;
