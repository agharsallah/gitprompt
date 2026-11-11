//! Extractors that turn a request into typed input, every failure an [`ApiError`] (422 for input
//! that doesn't validate, like FastAPI).

use axum::body::Bytes;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use gitprompt_store::filter::PromptFilter;

use crate::error::ApiError;
use crate::model::{ExportQuery, PromptQuery, StatsQuery};
use crate::query;

fn pairs(parts: &Parts) -> Vec<(String, String)> {
    query::pairs(parts.uri.query())
}

/// The prompt filters of the query string.
pub(super) struct Filter(pub PromptFilter);

impl<S: Send + Sync> FromRequestParts<S> for Filter {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        query::prompt_filter(&pairs(parts)).map(Filter)
    }
}

/// Filters plus `limit` (`/api/prompts`, `/api/search`).
pub(super) struct Prompts(pub PromptQuery);

impl<S: Send + Sync> FromRequestParts<S> for Prompts {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        query::prompt_query(&pairs(parts)).map(Prompts)
    }
}

/// Filters plus grouping (`/api/stats`).
pub(super) struct Grouping(pub StatsQuery);

impl<S: Send + Sync> FromRequestParts<S> for Grouping {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        query::stats_query(&pairs(parts)).map(Grouping)
    }
}

/// Export options and the prompt filters.
pub(super) struct ExportParams(pub ExportQuery, pub PromptFilter);

impl<S: Send + Sync> FromRequestParts<S> for ExportParams {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        let pairs = pairs(parts);
        Ok(ExportParams(query::export_query(&pairs)?, query::prompt_filter(&pairs)?))
    }
}

/// `dry_run`, `DEFAULT` when not given.
pub(super) struct DryRun<const DEFAULT: bool>(pub bool);

impl<S: Send + Sync, const DEFAULT: bool> FromRequestParts<S> for DryRun<DEFAULT> {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        query::dry_run(&pairs(parts), DEFAULT).map(DryRun)
    }
}

/// Path parameters; one that doesn't parse (a prompt id that isn't a number) is a 422.
pub(super) struct Params<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequestParts<S> for Params<T> {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(value)| Params(value))
            .map_err(|e| ApiError::Unprocessable(e.body_text()))
    }
}

/// A JSON body, whatever the content type says; missing, malformed or of the wrong shape is a 422.
pub(super) struct Body<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for Body<T> {
    type Rejection = ApiError;
    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        let bytes = Bytes::from_request(request, state).await.map_err(|e| ApiError::Unprocessable(e.body_text()))?;
        if bytes.is_empty() {
            return Err(ApiError::Unprocessable("a JSON body is required".into()));
        }
        serde_json::from_slice(&bytes).map(Body).map_err(|e| ApiError::Unprocessable(format!("body: {e}")))
    }
}
