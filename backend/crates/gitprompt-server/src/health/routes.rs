//! Per-route request statistics under normalized keys (`GET /api/prompts/{id}`), bounded in size.

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};

use super::stats::percentile;
use crate::access_log::is_slow;

/// Recent durations kept per route for the percentiles.
const RECENT: usize = 256;
/// Distinct route keys recorded; later new keys count under [`OTHER`].
const MAX_ROUTES: usize = 128;
const OTHER: &str = "(other)";

/// One route in the status.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteOut {
    /// `GET /api/prompts/{id}`.
    pub route: String,
    pub count: u64,
    /// Answered 5xx.
    pub errors: u64,
    /// Slower than the access log's limit.
    pub slow: u64,
    /// Over the most recent requests (up to 256).
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Default)]
struct RouteStats {
    count: u64,
    errors: u64,
    slow: u64,
    max_ms: f64,
    recent: VecDeque<f64>,
}

impl RouteStats {
    fn add(&mut self, status: u16, ms: f64) {
        self.count += 1;
        self.errors += u64::from(status >= 500);
        self.slow += u64::from(is_slow(ms));
        self.max_ms = self.max_ms.max(ms);
        if self.recent.len() == RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back(ms);
    }

    fn out(&self, route: &str) -> RouteOut {
        let mut recent: Vec<f64> = self.recent.iter().copied().collect();
        recent.sort_by(f64::total_cmp);
        RouteOut {
            route: route.to_owned(),
            count: self.count,
            errors: self.errors,
            slow: self.slow,
            p50_ms: tenths(percentile(&recent, 0.50)),
            p95_ms: tenths(percentile(&recent, 0.95)),
            max_ms: tenths(self.max_ms),
        }
    }
}

/// Every route seen since start.
#[derive(Debug, Default)]
pub(super) struct RouteTable {
    routes: HashMap<String, RouteStats>,
}

impl RouteTable {
    pub(super) fn record(&mut self, method: &str, path: &str, status: u16, ms: f64) {
        let mut key = format!("{method} {}", route_of(path));
        if !self.routes.contains_key(&key) && self.routes.len() >= MAX_ROUTES {
            OTHER.clone_into(&mut key);
        }
        self.routes.entry(key).or_default().add(status, ms);
    }

    /// Requests answered 5xx and requests slower than the limit, so far.
    pub(super) fn totals(&self) -> (u64, u64) {
        self.routes.values().fold((0, 0), |(errors, slow), s| (errors + s.errors, slow + s.slow))
    }

    /// The `limit` routes with the slowest p95.
    pub(super) fn slowest(&self, limit: usize) -> Vec<RouteOut> {
        let mut out: Vec<RouteOut> = self.routes.iter().map(|(route, stats)| stats.out(route)).collect();
        out.sort_by(|a, b| b.p95_ms.total_cmp(&a.p95_ms).then_with(|| a.route.cmp(&b.route)));
        out.truncate(limit);
        out
    }
}

/// The route a path belongs to: API and ingest paths with ids folded to `{id}`; every other path
/// is a dashboard file.
pub fn route_of(path: &str) -> String {
    let api = ["/api/", "/ingest/"].iter().any(|prefix| path.starts_with(prefix)) || path == "/healthz";
    if !api {
        return "(dashboard)".to_owned();
    }
    path.split('/').map(|segment| if is_id(segment) { "{id}" } else { segment }).collect::<Vec<_>>().join("/")
}

/// A number, or a long token with digits (UUIDs, hashes): never part of a route's shape.
fn is_id(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    let number = !bytes.is_empty() && bytes.iter().all(u8::is_ascii_digit);
    let token = bytes.len() >= 16 && bytes.iter().any(u8::is_ascii_digit) && bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'-');
    number || token
}

fn tenths(ms: f64) -> f64 {
    (ms * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_fold_into_the_route_and_dashboard_files_share_one_key() {
        assert_eq!(route_of("/api/prompts/716"), "/api/prompts/{id}");
        assert_eq!(route_of("/api/stories/session/8a657b39-6048-4950-bf27-ddf7f7074df1"), "/api/stories/session/{id}");
        assert_eq!(route_of("/api/sources/claude-code/spec"), "/api/sources/claude-code/spec");
        assert_eq!(route_of("/ingest/claude-code-otel/v1/logs"), "/ingest/claude-code-otel/v1/logs");
        assert_eq!(route_of("/_app/immutable/chunks/abc123.js"), "(dashboard)");
    }

    #[test]
    fn percentiles_errors_and_slow_requests_are_counted() {
        let mut table = RouteTable::default();
        for ms in 1..=100 {
            table.record("GET", &format!("/api/prompts/{ms}"), 200, f64::from(ms));
        }
        table.record("GET", "/api/prompts/1", 500, 1500.0);
        let route = &table.slowest(1)[0];
        assert_eq!((route.route.as_str(), route.count, route.errors, route.slow), ("GET /api/prompts/{id}", 101, 1, 1));
        assert_eq!((route.p50_ms, route.p95_ms, route.max_ms), (51.0, 96.0, 1500.0));
        assert_eq!(table.totals(), (1, 1));
    }

    #[test]
    fn the_table_is_bounded() {
        let mut table = RouteTable::default();
        for i in 0..(MAX_ROUTES + 10) {
            table.record("GET", &format!("/api/x{i}"), 200, 1.0);
        }
        assert_eq!(table.routes.len(), MAX_ROUTES + 1, "the extra routes share one key");
    }
}
