//! The database, backups and clean-ups that preview first (port of `api/routes/data.py`).

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::extract::{Body, DryRun};
use crate::error::ApiResult;
use crate::model::{CompactIn, DataOut, DeleteIn, ReportOut};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/data", get(data))
}

pub(super) fn writes() -> Router<Service> {
    Router::new().route("/api/data/compact", post(compact)).route("/api/data/delete-events", post(delete_events))
}

async fn data(State(service): State<Service>) -> ApiResult<Json<DataOut>> {
    service.data().await.map(Json)
}

async fn compact(State(service): State<Service>, DryRun(dry_run): DryRun<true>, Body(body): Body<CompactIn>) -> ApiResult<Json<ReportOut>> {
    service.compact(body, dry_run).await.map(Json)
}

async fn delete_events(
    State(service): State<Service>,
    DryRun(dry_run): DryRun<true>,
    Body(body): Body<DeleteIn>,
) -> ApiResult<Json<ReportOut>> {
    service.delete_events(body, dry_run).await.map(Json)
}
