//! Errors of the API and the service facade, and their one HTTP mapping: a JSON body
//! `{"detail": "<message>"}` with the status of [`ApiError::status`]. Domain errors keep the status
//! `gitprompt_core::Error::status` gives them.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// From the core crates: status from `gitprompt_core::Error::status`.
    #[error(transparent)]
    Domain(#[from] gitprompt_core::Error),
    /// A request parameter or body that doesn't validate (422), e.g. `limit=0`, `origin=nope`,
    /// `group_by=nope`, `split_by` without `group_by`, a session export without `id`.
    #[error("{0}")]
    Unprocessable(String),
    /// Ingest without the receiver token, or with a wrong one (401).
    #[error("missing or wrong token")]
    Unauthorized,
    /// A write without `X-GitPrompt: 1` (403): browsers only send a custom header from our own
    /// origin, so cross-site pages can't change anything.
    #[error("missing X-GitPrompt header")]
    MissingDashboardHeader,
    /// `Host` is not 127.0.0.1 or localhost (400): DNS-rebinding defence.
    #[error("Invalid host header")]
    UntrustedHost,
    /// Something named in the path doesn't exist and isn't a domain object (404), e.g. an export
    /// chart id or an unknown route.
    #[error("{0}")]
    NotFound(String),
    /// A failure outside the domain (500): a background task that panicked or was cancelled.
    #[error("{0}")]
    Internal(String),
}

impl ApiError {
    pub fn status(&self) -> u16 {
        match self {
            ApiError::Domain(e) => e.status(),
            ApiError::Unprocessable(_) => 422,
            ApiError::Unauthorized => 401,
            ApiError::MissingDashboardHeader => 403,
            ApiError::UntrustedHost => 400,
            ApiError::NotFound(_) => 404,
            ApiError::Internal(_) => 500,
        }
    }

    /// What a Tauri command returns as its error, and the HTTP body (without `status`).
    pub fn body(&self) -> ErrorBody {
        ErrorBody { status: self.status(), detail: self.to_string() }
    }
}

/// `{"status": 404, "detail": "prompt not found"}`. HTTP answers serialize `detail` only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub status: u16,
    pub detail: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        if status.is_server_error() {
            tracing::error!(error = %self, "request failed");
        }
        (status, Json(serde_json::json!({ "detail": self.to_string() }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
