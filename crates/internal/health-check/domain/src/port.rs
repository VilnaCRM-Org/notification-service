//! The port that every dependency probe implements.

use shared_domain::BoxError;

use crate::value_object::dependency::Dependency;

/// Checks that one dependency of the service answers.
#[async_trait::async_trait]
pub trait HealthProbe: std::fmt::Debug + Send + Sync {
    /// The dependency this probe checks.
    fn dependency(&self) -> Dependency;

    /// Probes the dependency.
    ///
    /// # Errors
    ///
    /// Returns the cause when the dependency does not answer correctly.
    async fn check(&self) -> Result<(), BoxError>;
}
