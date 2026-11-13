//! Wire shapes of the example resource: the response, the create request and the list parameters.
//!
//! Schema constraints and validator attributes sit on the same field so that both stay equal.

use serde::{Deserialize, Serialize};
use shared_domain::PageRequest;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::query::ExampleApiResourceView;
use crate::query::list_example_api_resources::ListExampleApiResources;

/// The defaults of the shared kernel, signed like the parameters; both fit in an `i32`.
const DEFAULT_PAGE: i32 = PageRequest::DEFAULT_PAGE.cast_signed();
const DEFAULT_ITEMS_PER_PAGE: i32 = PageRequest::DEFAULT_ITEMS_PER_PAGE.cast_signed();

/// Template example resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = ExampleApiResource)]
pub struct ExampleApiResourceResponse {
    /// Identifier of the resource.
    pub id: uuid::Uuid,
    /// Display name of the resource.
    #[schema(min_length = 1, max_length = 255, pattern = r"^[^\u0000]*$")]
    pub name: String,
    /// Free-text description of the resource.
    #[schema(min_length = 1, max_length = 1024, pattern = r"^[^\u0000]*$")]
    pub description: String,
}

impl From<ExampleApiResourceView> for ExampleApiResourceResponse {
    fn from(view: ExampleApiResourceView) -> Self {
        Self {
            id: view.id.as_uuid(),
            name: view.name,
            description: view.description,
        }
    }
}

/// Body of the create operation.
#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
#[serde(deny_unknown_fields)]
pub struct CreateExampleApiResourceRequest {
    /// Display name of the resource.
    #[schema(
        min_length = 1,
        max_length = 255,
        pattern = r"^[^\u0000]*$",
        example = "Example name"
    )]
    #[validate(
        length(
            min = 1,
            max = 255,
            message = "This value must contain between 1 and 255 characters."
        ),
        does_not_contain(
            pattern = "\0",
            message = "This value must not contain the NUL character."
        )
    )]
    pub name: String,
    /// Free-text description of the resource.
    #[schema(
        min_length = 1,
        max_length = 1024,
        pattern = r"^[^\u0000]*$",
        example = "Example description"
    )]
    #[validate(
        length(
            min = 1,
            max = 1024,
            message = "This value must contain between 1 and 1024 characters."
        ),
        does_not_contain(
            pattern = "\0",
            message = "This value must not contain the NUL character."
        )
    )]
    pub description: String,
}

/// Query parameters of the list operation; signed so that a negative value is a 422, not a parse
/// error (AD-22).
#[derive(Debug, Clone, Copy, Deserialize, IntoParams, Validate)]
#[serde(rename_all = "camelCase", default)]
#[into_params(parameter_in = Query)]
pub struct ListExampleApiResourcesParams {
    /// Page number, starting at 1.
    #[param(minimum = 1, default = 1)]
    #[validate(range(min = 1, message = "This value must be 1 or greater."))]
    pub page: i32,
    /// Number of items per page.
    #[param(minimum = 1, maximum = 100, default = 30)]
    #[validate(range(min = 1, max = 100, message = "This value must be between 1 and 100."))]
    pub items_per_page: i32,
}

impl Default for ListExampleApiResourcesParams {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            items_per_page: DEFAULT_ITEMS_PER_PAGE,
        }
    }
}

impl ListExampleApiResourcesParams {
    /// The query for these parameters; call it after validation, which makes both values positive.
    #[must_use]
    pub const fn into_query(self) -> ListExampleApiResources {
        ListExampleApiResources {
            page: self.page.unsigned_abs(),
            items_per_page: self.items_per_page.unsigned_abs(),
        }
    }
}

#[cfg(test)]
mod tests;
