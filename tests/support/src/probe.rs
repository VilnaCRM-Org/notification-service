//! Health probe double.

use async_trait::async_trait;
use health_check_domain::{Dependency, HealthProbe};
use shared_domain::BoxError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Behaviour {
    Healthy,
    Failing,
    Hanging,
    Panicking,
}

/// A probe of one dependency with a fixed behaviour.
#[derive(Debug)]
pub struct StubProbe {
    dependency: Dependency,
    behaviour: Behaviour,
}

impl StubProbe {
    const fn with(dependency: Dependency, behaviour: Behaviour) -> Self {
        Self {
            dependency,
            behaviour,
        }
    }

    /// A probe that succeeds.
    #[must_use]
    pub const fn healthy(dependency: Dependency) -> Self {
        Self::with(dependency, Behaviour::Healthy)
    }

    /// A probe that returns an error.
    #[must_use]
    pub const fn failing(dependency: Dependency) -> Self {
        Self::with(dependency, Behaviour::Failing)
    }

    /// A probe that never completes.
    #[must_use]
    pub const fn hanging(dependency: Dependency) -> Self {
        Self::with(dependency, Behaviour::Hanging)
    }

    /// A probe that panics.
    #[must_use]
    pub const fn panicking(dependency: Dependency) -> Self {
        Self::with(dependency, Behaviour::Panicking)
    }
}

#[async_trait]
impl HealthProbe for StubProbe {
    fn dependency(&self) -> Dependency {
        self.dependency
    }

    #[expect(clippy::panic, reason = "the panicking probe exists to panic")]
    async fn check(&self) -> Result<(), BoxError> {
        match self.behaviour {
            Behaviour::Healthy => Ok(()),
            Behaviour::Failing => Err("stub probe failure".into()),
            Behaviour::Hanging => std::future::pending().await,
            Behaviour::Panicking => panic!("stub probe panic"),
        }
    }
}

#[cfg(test)]
mod tests;
