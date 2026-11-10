//! The machine-readable state of this daemon (new in the Rust port; `gitprompt doctor` shows more).

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::error::ApiResult;
use crate::service::Service;
use crate::status::StatusOut;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/status", get(status))
}

async fn status(State(service): State<Service>) -> ApiResult<Json<StatusOut>> {
    service.status().await.map(Json)
}
