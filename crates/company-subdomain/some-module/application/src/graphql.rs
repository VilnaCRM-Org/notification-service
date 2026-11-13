//! GraphQL adapter of the example context: the object types and the query and mutation roots.
//!
//! The resolvers validate through the REST DTOs and call the same command and queries as the REST
//! handlers, so both transports apply one set of rules. Every failure becomes a problem first and
//! a GraphQL error second.

use async_graphql::{Context, ID, InputObject, Object, SimpleObject};
use shared_application::graphql::problem_to_error;
use shared_application::http::{Problem, Violation};
use shared_domain::Uuid;
use validator::Validate as _;

use crate::http::SomeModuleState;
use crate::http::dto::{CreateExampleApiResourceRequest, ListExampleApiResourcesParams};
use crate::http::handler::{create_resource, find_resource, list_resources};
use crate::query::ExampleApiResourceView;

/// Template example resource.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "ExampleApiResource")]
pub struct ExampleApiResourceObject {
    /// Identifier.
    pub id: ID,
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
}

impl From<ExampleApiResourceView> for ExampleApiResourceObject {
    fn from(view: ExampleApiResourceView) -> Self {
        Self {
            id: ID::from(view.id.to_string()),
            name: view.name,
            description: view.description,
        }
    }
}

/// Input of the create mutation.
#[derive(Debug, InputObject)]
pub struct CreateExampleApiResourceInput {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
}

/// The schema was built without the `SomeModuleState` data.
#[derive(Debug, thiserror::Error)]
#[error("the GraphQL schema has no SomeModuleState data")]
struct MissingState;

fn state<'ctx>(ctx: &Context<'ctx>) -> async_graphql::Result<&'ctx SomeModuleState> {
    ctx.data::<SomeModuleState>()
        .map_err(|_error| problem_to_error(Problem::internal(&MissingState)))
}

fn parse_id(id: &ID) -> Result<Uuid, Problem> {
    Uuid::parse(id.as_str()).map_err(|_error| {
        Problem::validation_failed(vec![Violation {
            property_path: "id".to_owned(),
            message: "This value is not a valid UUID.".to_owned(),
            code: "uuid".to_owned(),
        }])
    })
}

async fn find(
    state: &SomeModuleState,
    id: &ID,
) -> Result<Option<ExampleApiResourceObject>, Problem> {
    let found = find_resource(state, parse_id(id)?).await?;
    Ok(found.map(ExampleApiResourceObject::from))
}

async fn list(
    state: &SomeModuleState,
    params: ListExampleApiResourcesParams,
) -> Result<Vec<ExampleApiResourceObject>, Problem> {
    params.validate()?;
    let views = list_resources(state, params).await?;
    Ok(views
        .into_iter()
        .map(ExampleApiResourceObject::from)
        .collect())
}

async fn create(
    state: &SomeModuleState,
    input: CreateExampleApiResourceInput,
) -> Result<ExampleApiResourceObject, Problem> {
    let request = CreateExampleApiResourceRequest {
        name: input.name,
        description: input.description,
    };
    request.validate()?;
    Ok(ExampleApiResourceObject::from(
        create_resource(state, request).await?,
    ))
}

/// Query root of the example context.
#[derive(Debug, Default)]
pub struct ExampleApiResourceQuery;

#[Object]
impl ExampleApiResourceQuery {
    /// Retrieves an example API resource by id.
    async fn example_api_resource(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> async_graphql::Result<Option<ExampleApiResourceObject>> {
        find(state(ctx)?, &id).await.map_err(problem_to_error)
    }

    /// Retrieves one page of example API resources, newest first.
    async fn example_api_resources(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 1, desc = "Page number, from 1.")] page: i32,
        #[graphql(default = 30, desc = "Number of items per page, from 1 to 100.")]
        items_per_page: i32,
    ) -> async_graphql::Result<Vec<ExampleApiResourceObject>> {
        let params = ListExampleApiResourcesParams {
            page,
            items_per_page,
        };
        list(state(ctx)?, params).await.map_err(problem_to_error)
    }
}

/// Mutation root of the example context.
#[derive(Debug, Default)]
pub struct ExampleApiResourceMutation;

#[Object]
impl ExampleApiResourceMutation {
    /// Creates an example API resource.
    async fn create_example_api_resource(
        &self,
        ctx: &Context<'_>,
        input: CreateExampleApiResourceInput,
    ) -> async_graphql::Result<ExampleApiResourceObject> {
        create(state(ctx)?, input).await.map_err(problem_to_error)
    }
}

#[cfg(test)]
mod tests;
