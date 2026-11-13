//! REST handlers of the example resource, and the use-case calls that the GraphQL resolvers share.

use axum::Json;
use axum::extract::State;
use axum::http::header::LOCATION;
use axum::http::{HeaderName, StatusCode};
use shared_application::http::{ApiPath, Problem, ValidatedJson, ValidatedQuery};
use shared_domain::Uuid;

use super::SomeModuleState;
use super::dto::{
    CreateExampleApiResourceRequest, ExampleApiResourceResponse, ListExampleApiResourcesParams,
};
use crate::command::create_example_api_resource::CreateExampleApiResource;
use crate::error::ExampleApiResourceError;
use crate::query::ExampleApiResourceView;
use crate::query::get_example_api_resource::GetExampleApiResource;

/// Creates a resource from a validated request and reads it back.
pub(crate) async fn create_resource(
    state: &SomeModuleState,
    request: CreateExampleApiResourceRequest,
) -> Result<ExampleApiResourceView, Problem> {
    let id = state.uuid_generator.generate();
    state
        .command_bus
        .dispatch(CreateExampleApiResource {
            id,
            name: request.name,
            description: request.description,
        })
        .await?;
    find_resource(state, id).await?.ok_or_else(|| {
        Problem::from(ExampleApiResourceError::Storage(
            "created resource is missing".into(),
        ))
    })
}

/// Reads one resource, `None` when it does not exist.
pub(crate) async fn find_resource(
    state: &SomeModuleState,
    id: Uuid,
) -> Result<Option<ExampleApiResourceView>, Problem> {
    Ok(state.query_bus.ask(GetExampleApiResource { id }).await?)
}

/// Reads one page of resources for validated parameters.
pub(crate) async fn list_resources(
    state: &SomeModuleState,
    params: ListExampleApiResourcesParams,
) -> Result<Vec<ExampleApiResourceView>, Problem> {
    Ok(state.query_bus.ask(params.into_query()).await?)
}

/// Creates an `ExampleApiResource` resource.
///
/// # Errors
///
/// A problem document: 500 when the command or the read-back fails.
#[utoipa::path(
    post,
    path = "/example_api_resources",
    operation_id = "api_example_api_resources_post",
    tag = "ExampleApiResource",
    summary = "Creates an ExampleApiResource resource.",
    description = "Validates the input, stores the resource and returns it.",
    request_body(content = CreateExampleApiResourceRequest, content_type = "application/json"),
    responses(
        (
            status = 201,
            description = "ExampleApiResource resource created",
            body = ExampleApiResourceResponse,
            headers(("location" = String, description = "Path of the created resource."))
        ),
        (status = 400, description = "Invalid input.", body = Problem, content_type = "application/problem+json"),
        (status = 413, description = "Request body too large.", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Unsupported media type.", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Validation failed.", body = Problem, content_type = "application/problem+json"),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn create(
    State(state): State<SomeModuleState>,
    ValidatedJson(body): ValidatedJson<CreateExampleApiResourceRequest>,
) -> Result<
    (
        StatusCode,
        [(HeaderName, String); 1],
        Json<ExampleApiResourceResponse>,
    ),
    Problem,
> {
    let created = ExampleApiResourceResponse::from(create_resource(&state, body).await?);
    let location = format!("/api/example_api_resources/{}", created.id);
    Ok((StatusCode::CREATED, [(LOCATION, location)], Json(created)))
}

/// Retrieves an `ExampleApiResource` resource.
///
/// # Errors
///
/// The 404 problem for an unknown id, the internal problem when the query fails.
#[utoipa::path(
    get,
    path = "/example_api_resources/{id}",
    operation_id = "api_example_api_resources_id_get",
    tag = "ExampleApiResource",
    summary = "Retrieves an ExampleApiResource resource.",
    description = "Returns the resource identified by id.",
    params(("id" = uuid::Uuid, Path, description = "ExampleApiResource identifier")),
    responses(
        (status = 200, description = "ExampleApiResource resource", body = ExampleApiResourceResponse),
        (status = 400, description = "Invalid identifier.", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "Resource not found.", body = Problem, content_type = "application/problem+json"),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get_one(
    State(state): State<SomeModuleState>,
    ApiPath(id): ApiPath<uuid::Uuid>,
) -> Result<Json<ExampleApiResourceResponse>, Problem> {
    find_resource(&state, Uuid::from_uuid(id))
        .await?
        .map(|view| Json(ExampleApiResourceResponse::from(view)))
        .ok_or_else(|| {
            Problem::new(StatusCode::NOT_FOUND)
                .with_detail(format!("ExampleApiResource {id} was not found."))
        })
}

/// Retrieves the collection of `ExampleApiResource` resources.
///
/// # Errors
///
/// The internal problem when the query fails.
#[utoipa::path(
    get,
    path = "/example_api_resources",
    operation_id = "api_example_api_resources_get",
    tag = "ExampleApiResource",
    summary = "Retrieves the collection of ExampleApiResource resources.",
    description = "Returns one page of resources, newest first.",
    params(ListExampleApiResourcesParams),
    responses(
        (status = 200, description = "ExampleApiResource collection", body = Vec<ExampleApiResourceResponse>),
        (status = 400, description = "Invalid query parameter.", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Validation failed.", body = Problem, content_type = "application/problem+json"),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list(
    State(state): State<SomeModuleState>,
    ValidatedQuery(params): ValidatedQuery<ListExampleApiResourcesParams>,
) -> Result<Json<Vec<ExampleApiResourceResponse>>, Problem> {
    let views = list_resources(&state, params).await?;
    Ok(Json(
        views
            .into_iter()
            .map(ExampleApiResourceResponse::from)
            .collect(),
    ))
}

#[cfg(test)]
mod tests;
