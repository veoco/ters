use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRef, FromRequest, FromRequestParts},
    http::{request::Parts, Request},
    Json,
};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use validator::Validate;

use super::errors::{AuthError, ValidateRequestError};
use super::models::User;
use super::utils::get_user;
use crate::AppState;

#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedJson<T>(pub T);

#[async_trait]
impl<T, S, B> FromRequest<S, B> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
    Json<T>: FromRequest<S, B, Rejection = JsonRejection>,
    B: Send + 'static,
{
    type Rejection = ValidateRequestError;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state).await?;
        value.validate()?;
        Ok(ValidatedJson(value))
    }
}

pub struct PMSubscriber(pub User);

#[async_trait]
impl<S> FromRequestParts<Arc<S>> for PMSubscriber
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<S>,
    ) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let user = get_user(parts, state).await?;
        let group = user.group.as_str();
        match group {
            "subscriber" | "contributor" | "editor" | "administrator" => {
                return Ok(PMSubscriber(user))
            }
            _ => return Err(AuthError::PermissionDeny),
        }
    }
}

pub struct PMContributor(pub User);

#[async_trait]
impl<S> FromRequestParts<Arc<S>> for PMContributor
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<S>,
    ) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let user = get_user(parts, state).await?;
        let group = user.group.as_str();
        match group {
            "contributor" | "editor" | "administrator" => return Ok(PMContributor(user)),
            _ => return Err(AuthError::PermissionDeny),
        }
    }
}

pub struct PMEditor(pub User);

#[async_trait]
impl<S> FromRequestParts<Arc<S>> for PMEditor
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<S>,
    ) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let user = get_user(parts, state).await?;
        let group = user.group.as_str();
        match group {
            "editor" | "administrator" => return Ok(PMEditor(user)),
            _ => return Err(AuthError::PermissionDeny),
        }
    }
}

pub struct PMAdministrator(pub User);

#[async_trait]
impl<S> FromRequestParts<Arc<S>> for PMAdministrator
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<S>,
    ) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let user = get_user(parts, state).await?;
        let group = user.group.as_str();
        match group {
            "administrator" => return Ok(PMAdministrator(user)),
            _ => return Err(AuthError::PermissionDeny),
        }
    }
}