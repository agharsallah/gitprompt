//! Possible leaks: the board of a range, and your decision about one finding.

use axum::extract::State;
use axum::routing::{get, put};
use axum::{Json, Router};

use super::extract::{Body, Filter, Params};
use crate::error::ApiResult;
use crate::model::{LeakIn, LeakSaved, Leaks};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/leaks", get(leaks))
}

pub(super) fn writes() -> Router<Service> {
    Router::new().route("/api/leaks/{id}", put(decide))
}

async fn leaks(State(service): State<Service>, Filter(filter): Filter) -> ApiResult<Json<Leaks>> {
    service.leaks(filter).await.map(Json)
}

async fn decide(State(service): State<Service>, Params(id): Params<String>, Body(body): Body<LeakIn>) -> ApiResult<Json<LeakSaved>> {
    service.set_leak(id, body).await.map(Json)
}
