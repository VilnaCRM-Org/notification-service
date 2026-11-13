//! The four security headers of every response (AD-14).
//!
//! Each header is set with `if_not_present`, so a handler can supply a page-specific value, for
//! instance the policy of the documentation page.

use axum::http::HeaderValue;
use axum::http::header::{
    CONTENT_SECURITY_POLICY, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use tower::ServiceBuilder;
use tower::layer::util::{Identity, Stack};
use tower_http::set_header::SetResponseHeaderLayer;

type Header = SetResponseHeaderLayer<HeaderValue>;

/// The four layers as one type.
pub type SecurityHeaders = Stack<Header, Stack<Header, Stack<Header, Stack<Header, Identity>>>>;

/// Default value of `x-content-type-options`.
pub const CONTENT_TYPE_OPTIONS: &str = "nosniff";

/// Default value of `x-frame-options`.
pub const FRAME_OPTIONS: &str = "DENY";

/// Default value of `referrer-policy`.
pub const REFERRER: &str = "no-referrer";

/// Default value of `content-security-policy`.
pub const DEFAULT_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; frame-ancestors 'none'";

/// The four `if_not_present` layers.
#[must_use]
pub fn layer() -> ServiceBuilder<SecurityHeaders> {
    ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::if_not_present(
            X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static(CONTENT_TYPE_OPTIONS),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            X_FRAME_OPTIONS,
            HeaderValue::from_static(FRAME_OPTIONS),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            REFERRER_POLICY,
            HeaderValue::from_static(REFERRER),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(DEFAULT_CONTENT_SECURITY_POLICY),
        ))
}

#[cfg(test)]
mod tests;
