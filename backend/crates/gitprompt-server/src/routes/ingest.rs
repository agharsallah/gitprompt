//! Receivers for hook calls and OpenTelemetry exports (port of `api/routes/ingest.py`). Hooks are
//! never blocked: whatever the pipeline does, the answer is the usual empty one; only an unknown
//! source (or a non-OTLP source on the OTLP path) is refused.

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;

use gitprompt_core::{Json, JsonMap};

use super::extract::{Body, Params};
use crate::error::ApiResult;
use crate::service::{Service, Signal};

pub(super) fn router() -> Router<Service> {
    Router::new().route("/ingest/{source}", post(ingest)).route("/ingest/{source}/v1/{signal}", post(ingest_otlp))
}

/// The answer whatever was stored; a failed capture is logged, never shown to the tool.
fn answered(source: &str, result: ApiResult<crate::bus::Notice>, ok: Response) -> Response {
    match result {
        Ok(_) => ok,
        Err(e) if e.status() == 404 => e.into_response(),
        Err(e) => {
            tracing::error!(%source, error = %e, "capture failed; the hook got its usual answer");
            ok
        }
    }
}

async fn ingest(State(service): State<Service>, Params(source): Params<String>, Body(payload): Body<JsonMap>) -> Response {
    let result = service.ingest(&source, Json::Object(payload)).await;
    answered(&source, result, StatusCode::NO_CONTENT.into_response())
}

async fn ingest_otlp(
    State(service): State<Service>,
    Params((source, signal)): Params<(String, Signal)>,
    Body(payload): Body<JsonMap>,
) -> Response {
    let result = service.ingest_otlp(&source, signal, Json::Object(payload)).await;
    // An empty ExportServiceResponse: everything accepted.
    answered(&source, result, axum::Json(serde_json::json!({})).into_response())
}
