//! Application error of the example context and its conversion to a problem document.

use shared_application::http::{Problem, Violation};
use shared_domain::{BoxError, InvalidPageRequest};
use some_module_domain::{ExampleApiResourceId, InvalidExampleApiResource, RepositoryError};

/// What can go wrong in a command or a query of the example context.
#[derive(Debug, thiserror::Error)]
pub enum ExampleApiResourceError {
    /// A value of the resource breaks an invariant.
    #[error(transparent)]
    Invalid(#[from] InvalidExampleApiResource),
    /// The page request is outside its bounds.
    #[error(transparent)]
    InvalidPage(#[from] InvalidPageRequest),
    /// A resource with the identifier is already stored.
    #[error("ExampleApiResource {0} already exists.")]
    AlreadyExists(ExampleApiResourceId),
    /// The storage failed.
    #[error("the example API resource storage is unavailable")]
    Storage(#[source] BoxError),
}

impl ExampleApiResourceError {
    /// Wraps a repository failure as a storage failure.
    pub(crate) fn storage(error: RepositoryError) -> Self {
        Self::Storage(Box::new(error))
    }
}

impl From<ExampleApiResourceError> for Problem {
    fn from(error: ExampleApiResourceError) -> Self {
        match &error {
            ExampleApiResourceError::Invalid(invalid) => {
                violation(invalid.property_path(), invalid.code(), invalid.to_string())
            }
            ExampleApiResourceError::InvalidPage(invalid) => {
                violation(page_property(*invalid), "range", invalid.to_string())
            }
            ExampleApiResourceError::AlreadyExists(_) | ExampleApiResourceError::Storage(_) => {
                Self::internal(&error)
            }
        }
    }
}

const fn page_property(invalid: InvalidPageRequest) -> &'static str {
    match invalid {
        InvalidPageRequest::Page => "page",
        InvalidPageRequest::ItemsPerPage => "itemsPerPage",
    }
}

fn violation(property_path: &str, code: &str, message: String) -> Problem {
    Problem::validation_failed(vec![Violation {
        property_path: property_path.to_owned(),
        message,
        code: code.to_owned(),
    }])
}

#[cfg(test)]
mod tests;
