//! One log line per HTTP request: method, path, status and duration — never the query string (it
//! carries search text), the body, or any header (the receiver token travels in `Authorization`).
//!
//! At `debug` normally; at `warn` when a request takes longer than [`SLOW_REQUEST`], so slow
//! endpoints show up in the default logs. Target `gitprompt_server::http`.

use std::time::{Duration, Instant};

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Requests slower than this are logged at `warn`.
pub const SLOW_REQUEST: Duration = Duration::from_secs(1);

/// The middleware (`axum::middleware::from_fn`). Duration is until the response head is ready
/// (a live-update stream counts as answered when it opens).
pub(crate) async fn log_request(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed();
    let status = response.status().as_u16();
    let ms = elapsed.as_secs_f64() * 1000.0;
    if elapsed > SLOW_REQUEST {
        tracing::warn!(target: "gitprompt_server::http", %method, path, status, ms, "slow request");
    } else {
        tracing::debug!(target: "gitprompt_server::http", %method, path, status, ms, "request");
    }
    response
}
