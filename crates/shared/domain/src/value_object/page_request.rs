//! Page request value object.

/// One page of a collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    page: u32,
    items_per_page: u32,
}

/// A page request outside its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidPageRequest {
    /// The page number is below 1.
    #[error("page must be 1 or greater")]
    Page,
    /// The page size is outside 1 to 100.
    #[error("itemsPerPage must be between 1 and 100")]
    ItemsPerPage,
}

impl PageRequest {
    /// Default page number.
    pub const DEFAULT_PAGE: u32 = 1;
    /// Default page size.
    pub const DEFAULT_ITEMS_PER_PAGE: u32 = 30;
    /// Largest page size.
    pub const MAX_ITEMS_PER_PAGE: u32 = 100;

    /// Validates both numbers.
    ///
    /// # Errors
    ///
    /// Returns the bound that was violated: the page is checked first.
    pub fn new(page: u32, items_per_page: u32) -> Result<Self, InvalidPageRequest> {
        if page < 1 {
            return Err(InvalidPageRequest::Page);
        }
        if !(1..=Self::MAX_ITEMS_PER_PAGE).contains(&items_per_page) {
            return Err(InvalidPageRequest::ItemsPerPage);
        }
        Ok(Self {
            page,
            items_per_page,
        })
    }

    /// SQL limit: the page size.
    #[must_use]
    pub fn limit(self) -> i64 {
        i64::from(self.items_per_page)
    }

    /// SQL offset: `(page - 1) * items_per_page`, computed in `i64`, where it cannot overflow.
    #[must_use]
    pub fn offset(self) -> i64 {
        i64::from(self.page.saturating_sub(1)).saturating_mul(i64::from(self.items_per_page))
    }
}

#[cfg(test)]
mod tests;
