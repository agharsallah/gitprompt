//! Exports as files: the dashboard's preview and the download are the same response (port of
//! `api/routes/export.py`), and the prompts behind a branch as Markdown for a pull request.

use axum::extract::State;
use axum::http::{HeaderName, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};

use super::extract::{Branch, ExportParams, Params};
use crate::error::{ApiError, ApiResult};
use crate::model::{ChartInfo, ExportFile, ExportKind};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new()
        .route("/api/export/branch", get(branch))
        .route("/api/export/{kind}", get(render))
        .route("/api/export/{kind}/charts", get(charts))
}

/// Always Markdown, with the redaction defaults (and the prompt text): the block is pasted as is.
async fn branch(State(service): State<Service>, Branch(query): Branch) -> ApiResult<Response> {
    service.export_branch(query).await.map(file)
}

async fn render(
    State(service): State<Service>,
    Params(kind): Params<ExportKind>,
    ExportParams(query, filter): ExportParams,
) -> ApiResult<Response> {
    service.export(kind, query, filter).await.map(file)
}

async fn charts(
    State(service): State<Service>,
    Params(kind): Params<ExportKind>,
    ExportParams(query, filter): ExportParams,
) -> ApiResult<Json<Vec<ChartInfo>>> {
    service.export_charts(kind, query, filter).await.map(Json)
}

/// The file with its headers: never cached or sniffed, inline unless downloaded.
fn file(export: ExportFile) -> Response {
    let disposition = if export.attachment { "attachment" } else { "inline" };
    let mut headers = vec![
        (header::CACHE_CONTROL, "no-store".to_owned()),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        (header::CONTENT_TYPE, export.media_type),
        (header::CONTENT_DISPOSITION, format!("{disposition}; filename=\"{}\"", export.filename)),
    ];
    if let Some(csp) = export.csp {
        headers.push((header::CONTENT_SECURITY_POLICY, csp));
    }
    let mut response = export.body.into_response();
    for (name, value) in headers {
        match HeaderValue::from_str(&value) {
            Ok(value) => {
                response.headers_mut().insert::<HeaderName>(name, value);
            }
            // The file name is ASCII by construction; anything else is a bug worth a 500.
            Err(_) => return ApiError::Internal(format!("invalid {name} header")).into_response(),
        }
    }
    response
}
