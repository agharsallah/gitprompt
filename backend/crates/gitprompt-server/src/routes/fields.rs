//! The field catalog (port of `api/routes/fields.py`).

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::error::ApiResult;
use crate::model::Catalog;
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/fields", get(catalog))
}

async fn catalog(State(service): State<Service>) -> ApiResult<Json<Catalog>> {
    service.fields().await.map(Json)
}
