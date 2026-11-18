//! One search over prompts, sessions, projects and commits.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::extract::Find;
use crate::error::ApiResult;
use crate::model::FindResults;
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/find", get(find))
}

async fn find(State(service): State<Service>, Find(query): Find) -> ApiResult<Json<FindResults>> {
    service.find(query).await.map(Json)
}
