//! Application error of the event stream and its conversion to a problem document.

use shared_application::http::{Problem, Violation};
use shared_domain::InvalidPageRequest;

/// What can go wrong in a query of the event stream.
#[derive(Debug, thiserror::Error)]
pub enum EventStreamError {
    /// The page request is outside its bounds.
    #[error(transparent)]
    InvalidPage(#[from] InvalidPageRequest),
}

impl From<EventStreamError> for Problem {
    fn from(error: EventStreamError) -> Self {
        match error {
            EventStreamError::InvalidPage(invalid) => Self::validation_failed(vec![Violation {
                property_path: page_property(invalid).to_owned(),
                message: invalid.to_string(),
                code: "range".to_owned(),
            }]),
        }
    }
}

const fn page_property(invalid: InvalidPageRequest) -> &'static str {
    match invalid {
        InvalidPageRequest::Page => "page",
        InvalidPageRequest::ItemsPerPage => "itemsPerPage",
    }
}

#[cfg(test)]
mod tests;
