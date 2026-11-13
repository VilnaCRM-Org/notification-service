//! Health-check context, application layer: `HealthService`, which probes every dependency
//! concurrently under a per-probe timeout, and the HTTP adapter that serves the health endpoint.
//!
//! The use-case code does not import `axum` or `utoipa`; only the `http` module does.

pub mod http;
pub mod service;

pub use http::{HealthCheckState, router};
pub use service::HealthService;
