//! Bus errors, typed and type-erased.

/// A message or an error boxed as `Any`.
pub type ErasedMessage = Box<dyn std::any::Any + Send>;

/// Failure of a typed dispatch.
#[derive(Debug, thiserror::Error)]
pub enum BusError<E> {
    /// No handler is registered for the message.
    #[error("no handler is registered for `{message}`")]
    NotRegistered {
        /// Name of the message.
        message: &'static str,
    },
    /// The handler failed.
    #[error(transparent)]
    Handler(E),
}

/// Failure crossing the dyn-compatible port.
#[derive(Debug)]
pub enum ErasedBusError {
    /// No handler is registered for the message.
    NotRegistered {
        /// Name of the message.
        message: &'static str,
    },
    /// The handler's error, boxed as `Any`.
    Handler(ErasedMessage),
}

impl ErasedBusError {
    pub(crate) fn into_typed<E: std::error::Error + Send + 'static>(
        self,
        message: &'static str,
    ) -> BusError<E> {
        match self {
            Self::NotRegistered { message } => BusError::NotRegistered { message },
            Self::Handler(error) => match error.downcast::<E>() {
                Ok(error) => BusError::Handler(*error),
                Err(_other) => BusError::NotRegistered { message },
            },
        }
    }
}

#[cfg(test)]
mod tests;
