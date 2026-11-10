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
use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;
use crate::service::Service;

/// Host names the server answers to.
pub const LOCAL_HOSTS: &[&str] = &["127.0.0.1", "localhost"];

/// The header writes must carry, and its value.
pub const DASHBOARD_HEADER: &str = "x-gitprompt";
pub const DASHBOARD_VALUE: &str = "1";

/// Whether a `Host` header value names a local host: the name before an optional `:port` is one
/// of [`LOCAL_HOSTS`].
pub fn is_local_host(host: &str) -> bool {
    let name = match host.rsplit_once(':') {
        Some((name, port)) if port.bytes().all(|b| b.is_ascii_digit()) => name,
        _ => host,
    };
    LOCAL_HOSTS.contains(&name)
}

/// `Ok` when `authorization` is exactly `Bearer <token>`; [`ApiError::Unauthorized`] otherwise
/// (missing header included). Constant-time in the token.
pub fn check_token(authorization: Option<&str>, token: &str) -> Result<(), ApiError> {
    let given = authorization.unwrap_or_default().as_bytes();
    let expected = format!("Bearer {token}");
    if constant_time_eq(given, expected.as_bytes()) { Ok(()) } else { Err(ApiError::Unauthorized) }
}

/// Equal bytes, comparing every byte of `expected` whatever `given` holds (only the lengths leak).
fn constant_time_eq(given: &[u8], expected: &[u8]) -> bool {
    let mut diff = u8::from(given.len() != expected.len());
    for (i, e) in expected.iter().enumerate() {
        diff |= given.get(i).copied().unwrap_or(!*e) ^ e;
    }
    diff == 0
}

/// `Ok` when the `X-GitPrompt` header is `1`; [`ApiError::MissingDashboardHeader`] otherwise.
pub fn check_dashboard(header: Option<&str>) -> Result<(), ApiError> {
    if header == Some(DASHBOARD_VALUE) { Ok(()) } else { Err(ApiError::MissingDashboardHeader) }
}

/// Wrap a router in the Host check (outermost, so it also covers the dashboard and `/healthz`).
pub fn trusted_hosts(router: Router) -> Router {
    router.layer(middleware::from_fn(host_check))
}

async fn host_check(request: Request, next: Next) -> Response {
    let host = request.headers().get(header::HOST).and_then(|v| v.to_str().ok());
    if host.is_some_and(is_local_host) { next.run(request).await } else { ApiError::UntrustedHost.into_response() }
}

/// Route layer of the ingest routes: the receiver token, read on every request.
pub(crate) async fn require_token(State(service): State<Service>, request: Request, next: Next) -> Response {
    let authorization = request.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    let checked = service.settings().token().map_err(ApiError::from).and_then(|token| check_token(authorization, &token));
    match checked {
        Ok(()) => next.run(request).await,
        Err(e) => e.into_response(),
    }
}

/// Route layer of every write route: `X-GitPrompt: 1`.
pub(crate) async fn require_dashboard(request: Request, next: Next) -> Response {
    let value = request.headers().get(DASHBOARD_HEADER).and_then(|v| v.to_str().ok());
    match check_dashboard(value) {
        Ok(()) => next.run(request).await,
        Err(e) => e.into_response(),
    }
}
