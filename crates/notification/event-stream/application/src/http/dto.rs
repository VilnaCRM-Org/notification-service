//! Wire shapes of the event catalog: the response and the list parameters.

use serde::{Deserialize, Serialize};
use shared_domain::PageRequest;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::query::EventTypeView;
use crate::query::list_event_types::ListEventTypes;

/// The defaults of the shared kernel, signed like the parameters; both fit in an `i32`.
const DEFAULT_PAGE: i32 = PageRequest::DEFAULT_PAGE.cast_signed();
const DEFAULT_ITEMS_PER_PAGE: i32 = PageRequest::DEFAULT_ITEMS_PER_PAGE.cast_signed();

/// A subscribable event type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = EventType)]
pub struct EventTypeResponse {
    /// Name of the event type.
    #[schema(
        min_length = 1,
        max_length = 128,
        pattern = r"^[a-z][a-z0-9]*([._-][a-z0-9]+)*$",
        example = "user.created"
    )]
    pub name: String,
}

impl From<EventTypeView> for EventTypeResponse {
    fn from(view: EventTypeView) -> Self {
        Self { name: view.name }
    }
}

/// Query parameters of the catalog; signed so that a negative value is a 422, not a parse error.
/// The bounds are checked by `PageRequest`, which reports the first violated one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, IntoParams, Validate)]
#[serde(rename_all = "camelCase", default)]
#[into_params(parameter_in = Query)]
pub struct ListEventTypesParams {
    /// Page number, starting at 1.
    #[param(minimum = 1, default = 1)]
    pub page: i32,
    /// Number of items per page.
    #[param(minimum = 1, maximum = 100, default = 30)]
    pub items_per_page: i32,
}

impl Default for ListEventTypesParams {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            items_per_page: DEFAULT_ITEMS_PER_PAGE,
        }
    }
}

impl ListEventTypesParams {
    /// The query for these parameters; a value below 1 becomes 0, which `PageRequest` refuses.
    #[must_use]
    pub fn into_query(self) -> ListEventTypes {
        ListEventTypes {
            page: u32::try_from(self.page).unwrap_or(0),
            items_per_page: u32::try_from(self.items_per_page).unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests;
