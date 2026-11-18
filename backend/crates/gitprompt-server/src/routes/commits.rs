//! A commit by its hash.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::extract::Params;
use crate::error::ApiResult;
use crate::model::CommitLookup;
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/commits/{hash}", get(commit))
}

async fn commit(State(service): State<Service>, Params(hash): Params<String>) -> ApiResult<Json<CommitLookup>> {
    service.commit(&hash).await.map(Json)
}
