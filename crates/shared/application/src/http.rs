//! HTTP adapters shared by every bounded context: the problem document, the problem-aware
//! extractors, the fallbacks and the request-budget middleware.

pub mod extract;
pub mod fallback;
pub mod middleware;
pub mod problem;

pub use extract::{ApiJson, ApiPath, ValidatedJson, ValidatedQuery};
pub use problem::{
    DependencyFailure, PROBLEM_CONTENT_TYPE, Problem, TYPE_BLANK, TYPE_DEPENDENCY_UNAVAILABLE,
    TYPE_VALIDATION_FAILED, Violation,
};
