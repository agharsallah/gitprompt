//! One log line per HTTP request: method, path, status and duration — never the query string (it
//! carries search text), the body, or any header (the receiver token travels in `Authorization`).
//!
//! At `debug` normally; at `warn` when a request takes longer than [`crate::health::SLOW_REQUEST`], so slow
//! endpoints show up in the default logs. Target `gitprompt_server::http`. Every request is also
//! recorded in [`Health`] (per-route counts, errors, percentiles).

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::health::{Health, is_slow};

/// The middleware (`axum::middleware::from_fn_with_state`). Duration is until the response head is
/// ready (a live-update stream counts as answered when it opens).
pub(crate) async fn log_request(State(health): State<Arc<Health>>, request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed();
    let status = response.status().as_u16();
    let ms = elapsed.as_secs_f64() * 1000.0;
    health.record(method.as_str(), &path, status, ms);
    if is_slow(ms) {
        tracing::warn!(target: "gitprompt_server::http", %method, path, status, ms, "slow request");
    } else {
        tracing::debug!(target: "gitprompt_server::http", %method, path, status, ms, "request");
    }
    response
}
