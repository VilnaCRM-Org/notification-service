//! The only extractors a handler may use for path, query and body: their rejections are problems.

use axum::Json;
use axum::extract::{FromRequest, FromRequestParts, Path, Query, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;
use validator::Validate;

use super::problem::Problem;

/// JSON body; a rejection is a problem document.
#[derive(Debug, FromRequest)]
#[from_request(via(Json), rejection(Problem))]
pub struct ApiJson<T>(pub T);

/// Path parameters; a rejection is a problem document.
#[derive(Debug, FromRequestParts)]
#[from_request(via(Path), rejection(Problem))]
pub struct ApiPath<T>(pub T);

/// JSON body that passed `Validate`; every violation is reported in one 422 problem.
#[derive(Debug)]
pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = Problem;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state).await?;
        value.validate()?;
        Ok(Self(value))
    }
}

/// Query string that passed `Validate`; every violation is reported in one 422 problem.
#[derive(Debug)]
pub struct ValidatedQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ValidatedQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state).await?;
        value.validate()?;
        Ok(Self(value))
    }
}

#[cfg(test)]
mod tests;
