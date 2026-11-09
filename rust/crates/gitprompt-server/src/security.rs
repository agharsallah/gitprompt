//! The request guards (port of `TrustedHostMiddleware` and `api/deps.py`):
//!
//! * every request: `Host` must name 127.0.0.1 or localhost (any port), else 400 — a web page
//!   that rebinds its DNS name to 127.0.0.1 still sends its own name;
//! * `/ingest/**`: `Authorization: Bearer <token>` (constant-time compare with
//!   `Settings::token`), else 401 — read on every request, so a rotated token applies at once;
//! * every write route (POST/PUT under `/api`): `X-GitPrompt: 1`, else 403.
//!
//! Read routes need neither header; CORS stays off.

use axum::Router;

use crate::error::ApiError;

/// Host names the server answers to.
pub const LOCAL_HOSTS: &[&str] = &["127.0.0.1", "localhost"];

/// The header writes must carry, and its value.
pub const DASHBOARD_HEADER: &str = "x-gitprompt";
pub const DASHBOARD_VALUE: &str = "1";

/// Whether a `Host` header value names a local host: the name before an optional `:port` is one
/// of [`LOCAL_HOSTS`].
pub fn is_local_host(_host: &str) -> bool {
    todo!()
}

/// `Ok` when `authorization` is exactly `Bearer <token>`; [`ApiError::Unauthorized`] otherwise
/// (missing header included). Constant-time in the token.
pub fn check_token(_authorization: Option<&str>, _token: &str) -> Result<(), ApiError> {
    todo!()
}

/// `Ok` when the `X-GitPrompt` header is `1`; [`ApiError::MissingDashboardHeader`] otherwise.
pub fn check_dashboard(_header: Option<&str>) -> Result<(), ApiError> {
    todo!()
}

/// Wrap a router in the Host check (outermost, so it also covers the dashboard and `/healthz`).
pub fn trusted_hosts(_router: Router) -> Router {
    todo!()
}
