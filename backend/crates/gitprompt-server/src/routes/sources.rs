//! Connectors: their facts, spec files, connect/disconnect with consent, enable/disable, remap
//! (port of `api/routes/sources.py`).

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::extract::{DryRun, Params};
use crate::error::ApiResult;
use crate::model::{Change, Enabled, RecentCapture, SourceOut, SpecProblemOut, SpecText, Updated};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new()
        .route("/api/sources", get(list))
        .route("/api/sources/recent", get(recent))
        .route("/api/sources/problems", get(problems))
        .route("/api/sources/{source}/spec", get(spec))
}

pub(super) fn writes() -> Router<Service> {
    Router::new()
        .route("/api/sources/{source}/connect", post(connect))
        .route("/api/sources/{source}/disconnect", post(disconnect))
        .route("/api/sources/{source}/enable", post(enable))
        .route("/api/sources/{source}/disable", post(disable))
        .route("/api/sources/{source}/remap", post(remap))
}

async fn list(State(service): State<Service>) -> ApiResult<Json<Vec<SourceOut>>> {
    service.sources().await.map(Json)
}

async fn recent(State(service): State<Service>) -> ApiResult<Json<Vec<RecentCapture>>> {
    service.recent_captures().await.map(Json)
}

async fn problems(State(service): State<Service>) -> ApiResult<Json<Vec<SpecProblemOut>>> {
    service.spec_problems().await.map(Json)
}

async fn spec(State(service): State<Service>, Params(source): Params<String>) -> ApiResult<Json<SpecText>> {
    service.spec_text(&source).await.map(Json)
}

async fn connect(
    State(service): State<Service>,
    Params(source): Params<String>,
    DryRun(dry_run): DryRun<false>,
) -> ApiResult<Json<Change>> {
    service.connect(&source, dry_run).await.map(Json)
}

async fn disconnect(
    State(service): State<Service>,
    Params(source): Params<String>,
    DryRun(dry_run): DryRun<false>,
) -> ApiResult<Json<Change>> {
    service.disconnect(&source, dry_run).await.map(Json)
}

async fn enable(State(service): State<Service>, Params(source): Params<String>) -> ApiResult<Json<Enabled>> {
    service.enable(&source).await.map(Json)
}

async fn disable(State(service): State<Service>, Params(source): Params<String>) -> ApiResult<Json<Enabled>> {
    service.disable(&source).await.map(Json)
}

async fn remap(State(service): State<Service>, Params(source): Params<String>) -> ApiResult<Json<Updated>> {
    service.remap(&source).await.map(Json)
}
