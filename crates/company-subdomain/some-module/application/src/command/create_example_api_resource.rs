//! Creates an `ExampleApiResource`.

use std::sync::Arc;

use shared_domain::{
    AggregateRoot as _, Clock, Command, CommandHandler, EventBus, SubscriberFailure, Uuid,
    UuidGenerator, error_chain,
};
use some_module_domain::{
    ExampleApiResource, ExampleApiResourceDescription, ExampleApiResourceId,
    ExampleApiResourceName, ExampleApiResourceRepository, RepositoryError,
};

use crate::error::ExampleApiResourceError;

/// Creates a resource with an identifier chosen by the caller.
#[derive(Debug, Clone)]
pub struct CreateExampleApiResource {
    /// Identifier of the new resource.
    pub id: Uuid,
    /// Name of the new resource.
    pub name: String,
    /// Description of the new resource.
    pub description: String,
}

impl Command for CreateExampleApiResource {
    type Error = ExampleApiResourceError;
    const NAME: &'static str = "CreateExampleApiResource";
}

/// Handles [`CreateExampleApiResource`].
#[derive(Debug)]
pub struct CreateExampleApiResourceHandler {
    repository: Arc<dyn ExampleApiResourceRepository>,
    event_bus: Arc<dyn EventBus>,
    clock: Arc<dyn Clock>,
    uuid_generator: Arc<dyn UuidGenerator>,
}

impl CreateExampleApiResourceHandler {
    /// A handler over its four ports.
    #[must_use]
    pub const fn new(
        repository: Arc<dyn ExampleApiResourceRepository>,
        event_bus: Arc<dyn EventBus>,
        clock: Arc<dyn Clock>,
        uuid_generator: Arc<dyn UuidGenerator>,
    ) -> Self {
        Self {
            repository,
            event_bus,
            clock,
            uuid_generator,
        }
    }

    async fn create(
        &self,
        command: CreateExampleApiResource,
    ) -> Result<(), ExampleApiResourceError> {
        let mut resource = self.build(command)?;
        self.store(&resource).await?;
        self.publish(&mut resource).await;
        Ok(())
    }

    fn build(
        &self,
        command: CreateExampleApiResource,
    ) -> Result<ExampleApiResource, ExampleApiResourceError> {
        let name = ExampleApiResourceName::new(command.name)?;
        let description = ExampleApiResourceDescription::new(command.description)?;
        Ok(ExampleApiResource::create(
            ExampleApiResourceId::new(command.id),
            name,
            description,
            self.uuid_generator.generate(),
            self.clock.now(),
        ))
    }

    async fn store(&self, resource: &ExampleApiResource) -> Result<(), ExampleApiResourceError> {
        match self.repository.save(resource).await {
            Ok(()) => Ok(()),
            Err(RepositoryError::AlreadyExists) => {
                Err(ExampleApiResourceError::AlreadyExists(resource.id()))
            }
            Err(other) => Err(ExampleApiResourceError::storage(other)),
        }
    }

    /// Publishes the recorded events; a failure is logged per subscriber and never fails the
    /// command, because the resource is already stored.
    async fn publish(&self, resource: &mut ExampleApiResource) {
        if let Err(error) = self.event_bus.publish(resource.pull_domain_events()).await {
            for failure in &error.failures {
                log_publication_failure(failure, &error_chain(&*failure.source));
            }
        }
    }
}

#[async_trait::async_trait]
impl CommandHandler<CreateExampleApiResource> for CreateExampleApiResourceHandler {
    async fn handle(
        &self,
        command: CreateExampleApiResource,
    ) -> Result<(), ExampleApiResourceError> {
        self.create(command).await
    }
}

fn log_publication_failure(failure: &SubscriberFailure, error: &str) {
    tracing::error!(event_name = failure.event_name, event_id = %failure.event_id, error = %error, "event publication failed");
}

#[cfg(test)]
mod tests;
