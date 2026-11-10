//! Statistics and session telemetry over the prompts the filters keep (port of
//! `api/routes/stats.py`).

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::extract::{Filter, Grouping};
use crate::error::ApiResult;
use crate::model::{Stats, Telemetry};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/stats", get(stats)).route("/api/stats/telemetry", get(telemetry))
}

async fn stats(State(service): State<Service>, Grouping(query): Grouping) -> ApiResult<Json<Stats>> {
    service.stats(query).await.map(Json)
}

async fn telemetry(State(service): State<Service>, Filter(filter): Filter) -> ApiResult<Json<Telemetry>> {
    service.telemetry(filter).await.map(Json)
}
