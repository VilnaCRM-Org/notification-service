//! REST adapter of the example context: the state its handlers read and the router that registers
//! them.

pub mod dto;
pub mod handler;

use std::sync::Arc;

use axum::extract::FromRef;
use shared_domain::{CommandBus, QueryBus, UuidGenerator};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// What the handlers of the example context need: the two buses and the identifier source.
#[derive(Debug, Clone)]
pub struct SomeModuleState {
    /// Dispatches the commands.
    pub command_bus: Arc<dyn CommandBus>,
    /// Answers the queries.
    pub query_bus: Arc<dyn QueryBus>,
    /// Chooses the identifier of a new resource.
    pub uuid_generator: Arc<dyn UuidGenerator>,
}

/// The routes of the example resource, to be nested under `/api`.
pub fn router<S>() -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
    SomeModuleState: FromRef<S>,
{
    OpenApiRouter::new()
        .routes(routes!(handler::list, handler::create))
        .routes(routes!(handler::get_one))
}
