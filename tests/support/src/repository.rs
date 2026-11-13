//! Repository double.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::{Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use shared_domain::{PageRequest, Uuid};
use some_module_domain::{
    ExampleApiResource, ExampleApiResourceDescription, ExampleApiResourceId,
    ExampleApiResourceName, ExampleApiResourceRepository, RepositoryError,
};

type Stored = (ExampleApiResourceName, ExampleApiResourceDescription);

/// A repository kept in a map, with the ordering and duplicate rule of the port.
#[derive(Debug)]
pub struct InMemoryExampleApiResourceRepository {
    available: bool,
    resources: Mutex<BTreeMap<Uuid, Stored>>,
}

impl InMemoryExampleApiResourceRepository {
    const fn with(available: bool) -> Self {
        Self {
            available,
            resources: Mutex::new(BTreeMap::new()),
        }
    }

    fn resources(&self) -> MutexGuard<'_, BTreeMap<Uuid, Stored>> {
        self.resources
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn ensure_available(&self) -> Result<(), RepositoryError> {
        if self.available {
            Ok(())
        } else {
            Err(RepositoryError::Unavailable(
                "in-memory repository failure".into(),
            ))
        }
    }

    /// An empty repository that works.
    #[must_use]
    pub const fn new() -> Self {
        Self::with(true)
    }

    /// A repository that returns `Unavailable` from every method.
    #[must_use]
    pub const fn failing() -> Self {
        Self::with(false)
    }
}

impl Default for InMemoryExampleApiResourceRepository {
    fn default() -> Self {
        Self::new()
    }
}

fn restore(id: Uuid, (name, description): &Stored) -> ExampleApiResource {
    ExampleApiResource::restore(
        ExampleApiResourceId::new(id),
        name.clone(),
        description.clone(),
    )
}

#[async_trait]
impl ExampleApiResourceRepository for InMemoryExampleApiResourceRepository {
    async fn save(&self, resource: &ExampleApiResource) -> Result<(), RepositoryError> {
        self.ensure_available()?;
        let id = resource.id().value();
        match self.resources().entry(id) {
            Entry::Occupied(_) => Err(RepositoryError::AlreadyExists),
            Entry::Vacant(slot) => {
                slot.insert((resource.name().clone(), resource.description().clone()));
                Ok(())
            }
        }
    }

    async fn find_by_id(
        &self,
        id: ExampleApiResourceId,
    ) -> Result<Option<ExampleApiResource>, RepositoryError> {
        self.ensure_available()?;
        Ok(self
            .resources()
            .get(&id.value())
            .map(|stored| restore(id.value(), stored)))
    }

    async fn find_page(
        &self,
        page: PageRequest,
    ) -> Result<Vec<ExampleApiResource>, RepositoryError> {
        self.ensure_available()?;
        let offset = usize::try_from(page.offset()).unwrap_or(usize::MAX);
        let limit = usize::try_from(page.limit()).unwrap_or(usize::MAX);
        Ok(self
            .resources()
            .iter()
            .rev()
            .skip(offset)
            .take(limit)
            .map(|(id, stored)| restore(*id, stored))
            .collect())
    }
}

#[cfg(test)]
mod tests;
