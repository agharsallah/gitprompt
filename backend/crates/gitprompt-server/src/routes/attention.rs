//! What needs you: the sessions to pick up and what is worth a look.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::extract::Filter;
use crate::error::ApiResult;
use crate::model::Attention;
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/attention", get(attention))
}

async fn attention(State(service): State<Service>, Filter(filter): Filter) -> ApiResult<Json<Attention>> {
    service.attention(filter).await.map(Json)
}
