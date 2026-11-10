//! User preferences (port of `api/routes/settings.py`).

use axum::extract::State;
use axum::routing::{get, put};
use axum::{Json, Router};

use gitprompt_core::JsonMap;

use super::extract::Body;
use crate::error::ApiResult;
use crate::model::{SettingsOut, SettingsSaved};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/settings", get(current))
}

pub(super) fn writes() -> Router<Service> {
    Router::new().route("/api/settings", put(update))
}

async fn current(State(service): State<Service>) -> ApiResult<Json<SettingsOut>> {
    service.preferences().await.map(Json)
}

async fn update(State(service): State<Service>, Body(changes): Body<JsonMap>) -> ApiResult<Json<SettingsSaved>> {
    service.update_preferences(changes).await.map(Json)
}
