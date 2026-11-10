//! The receiver token and its rotation (port of `api/routes/security.py`).

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::extract::DryRun;
use crate::error::ApiResult;
use crate::model::{Rotation, SecurityOut};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/security", get(security))
}

pub(super) fn writes() -> Router<Service> {
    Router::new().route("/api/security/rotate-token", post(rotate))
}

async fn security(State(service): State<Service>) -> ApiResult<Json<SecurityOut>> {
    service.security().await.map(Json)
}

async fn rotate(State(service): State<Service>, DryRun(dry_run): DryRun<true>) -> ApiResult<Json<Rotation>> {
    service.rotate_token(dry_run).await.map(Json)
}
