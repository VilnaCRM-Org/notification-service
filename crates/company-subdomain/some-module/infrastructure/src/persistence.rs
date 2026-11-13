//! PostgreSQL implementation of the example repository port: the SQL, the row mapping and the
//! translation of database errors.

use shared_domain::{PageRequest, Uuid};
use some_module_domain::{
    ExampleApiResource, ExampleApiResourceDescription, ExampleApiResourceId,
    ExampleApiResourceName, ExampleApiResourceRepository, RepositoryError,
};

/// Stores `ExampleApiResource` aggregates in the `example_api_resources` table.
#[derive(Debug)]
pub struct PgExampleApiResourceRepository {
    pool: sqlx::PgPool,
}

/// One stored row; it never leaves this module.
struct Row {
    id: uuid::Uuid,
    name: String,
    description: String,
}

impl PgExampleApiResourceRepository {
    /// Builds the repository over a pool; no connection is opened here.
    #[must_use]
    pub const fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    async fn insert(&self, resource: &ExampleApiResource) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO example_api_resources (id, name, description) VALUES ($1, $2, $3)",
            resource.id().value().as_uuid(),
            resource.name().as_str(),
            resource.description().as_str(),
        )
        .execute(&self.pool)
        .await
        .map(|_done| ())
        .map_err(translate)
    }

    async fn select_by_id(
        &self,
        id: ExampleApiResourceId,
    ) -> Result<Option<ExampleApiResource>, RepositoryError> {
        let row = sqlx::query_as!(
            Row,
            "SELECT id, name, description FROM example_api_resources WHERE id = $1",
            id.value().as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(translate)?;
        row.map(restore).transpose()
    }

    async fn select_page(
        &self,
        page: PageRequest,
    ) -> Result<Vec<ExampleApiResource>, RepositoryError> {
        let rows = sqlx::query_as!(
            Row,
            "SELECT id, name, description FROM example_api_resources ORDER BY id DESC LIMIT $1 OFFSET $2",
            page.limit(),
            page.offset(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(translate)?;
        rows.into_iter().map(restore).collect()
    }
}

/// SQLSTATE `23505` (the primary key is the only unique constraint) is a duplicate; everything else,
/// a pool acquire timeout included, is an unavailable repository.
fn translate(error: sqlx::Error) -> RepositoryError {
    let duplicate = error
        .as_database_error()
        .is_some_and(sqlx::error::DatabaseError::is_unique_violation);
    if duplicate {
        RepositoryError::AlreadyExists
    } else {
        RepositoryError::Unavailable(Box::new(error))
    }
}

/// Rebuilds the aggregate of a stored row; a row that breaks a domain invariant is unusable data.
fn restore(row: Row) -> Result<ExampleApiResource, RepositoryError> {
    let name = ExampleApiResourceName::new(row.name)
        .map_err(|error| RepositoryError::Unavailable(Box::new(error)))?;
    let description = ExampleApiResourceDescription::new(row.description)
        .map_err(|error| RepositoryError::Unavailable(Box::new(error)))?;
    Ok(ExampleApiResource::restore(
        ExampleApiResourceId::new(Uuid::from_uuid(row.id)),
        name,
        description,
    ))
}

#[async_trait::async_trait]
impl ExampleApiResourceRepository for PgExampleApiResourceRepository {
    async fn save(&self, resource: &ExampleApiResource) -> Result<(), RepositoryError> {
        self.insert(resource).await
    }

    async fn find_by_id(
        &self,
        id: ExampleApiResourceId,
    ) -> Result<Option<ExampleApiResource>, RepositoryError> {
        self.select_by_id(id).await
    }

    async fn find_page(
        &self,
        page: PageRequest,
    ) -> Result<Vec<ExampleApiResource>, RepositoryError> {
        self.select_page(page).await
    }
}

#[cfg(test)]
mod tests;
