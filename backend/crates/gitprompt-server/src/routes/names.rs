//! The names you give things: a session's, set and forgotten from its card and its story.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::put;
use axum::{Json, Router};

use super::extract::{Body, Params};
use crate::error::ApiResult;
use crate::model::{NameIn, NameOut};
use crate::service::Service;

pub(super) fn writes() -> Router<Service> {
    Router::new().route("/api/names/{kind}/{id}", put(set).delete(clear))
}

async fn set(
    State(service): State<Service>,
    Params((kind, id)): Params<(String, String)>,
    Body(body): Body<NameIn>,
) -> ApiResult<Json<NameOut>> {
    service.set_name(&kind, &id, &body.name).await.map(Json)
}

async fn clear(State(service): State<Service>, Params((kind, id)): Params<(String, String)>) -> ApiResult<StatusCode> {
    service.clear_name(&kind, &id).await.map(|()| StatusCode::NO_CONTENT)
}
