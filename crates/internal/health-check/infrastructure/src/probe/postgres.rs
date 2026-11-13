//! PostgreSQL probe.

use health_check_domain::{Dependency, HealthProbe};
use shared_domain::BoxError;
use sqlx::PgPool;

/// Checks that the database answers `SELECT 1` through the pool.
#[derive(Debug, Clone)]
pub struct PostgresProbe {
    pool: PgPool,
}

impl PostgresProbe {
    /// Creates a probe over the pool of the service.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Probe logic, kept out of the `#[async_trait]` body so coverage and complexity see it.
    async fn probe(&self) -> Result<(), BoxError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl HealthProbe for PostgresProbe {
    fn dependency(&self) -> Dependency {
        Dependency::Database
    }

    async fn check(&self) -> Result<(), BoxError> {
        self.probe().await
    }
}
