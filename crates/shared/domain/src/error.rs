//! Error alias used at port boundaries and the formatter of an error chain.

/// A boxed error that can cross threads.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Formats `cause` and every error behind it, outermost first, joined by `": "`.
///
/// Many SDK errors have a fixed top-level text (`service error`, `dispatch failure`) and keep the
/// real reason in their source, so a log entry that must say what went wrong formats the chain.
#[must_use]
pub fn error_chain(cause: &(dyn std::error::Error + 'static)) -> String {
    std::iter::successors(Some(cause), |error| error.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}

#[cfg(test)]
mod tests;
