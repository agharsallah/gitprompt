//! Local usage counts: the dashboard reports what gets used, Settings › Developer reads and clears it.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};

use super::extract::{Body, UsageDays};
use crate::error::ApiResult;
use crate::model::{UsageIn, UsageOut};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/usage", get(counts))
}

pub(super) fn writes() -> Router<Service> {
    Router::new().route("/api/usage", axum::routing::post(record).delete(clear))
}

async fn counts(State(service): State<Service>, UsageDays(days): UsageDays) -> ApiResult<Json<UsageOut>> {
    service.usage(days).await.map(Json)
}

async fn record(State(service): State<Service>, Body(body): Body<UsageIn>) -> ApiResult<StatusCode> {
    service.record_usage(body).await.map(|()| StatusCode::NO_CONTENT)
}

async fn clear(State(service): State<Service>) -> ApiResult<StatusCode> {
    service.clear_usage().await.map(|()| StatusCode::NO_CONTENT)
}
