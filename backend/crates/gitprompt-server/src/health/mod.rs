//! Early warning for performance problems: how each route behaves, how the database writer is
//! shared, and how much memory and CPU the process uses, with a `warn` log (target
//! `gitprompt_server::health`) when something crosses a limit. Reported under `health` in
//! `GET /api/status`, which `gitprompt doctor` turns into findings.
//!
//! * [`routes`]: every request, recorded by the access log under a normalized key.
//! * [`process`]: resident memory and CPU, sampled by the daemon every [`SAMPLE_EVERY`].
//! * [`monitor`]: the rules ([`Limits`]) that turn samples into warnings, once per cooldown.
//! * [`stats`]: percentiles and maxima, shared with the load benchmark.
//!
//! [`Health`] ties them together for the server. Dashboard-safe like the rest of the status:
//! paths and numbers only, never a query string.

pub mod monitor;
pub mod process;
pub mod routes;
pub mod stats;

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use gitprompt_store::WriterStats;

pub use monitor::{Limits, Monitor, Raised, Sample, WarningKind};
pub use process::{Peaks, ProcessSample, ProcessSampler};
use routes::RouteTable;
pub use routes::{RouteOut, route_of};
pub use stats::{max_present, percentile};

/// How often the daemon samples the process.
pub const SAMPLE_EVERY: Duration = Duration::from_secs(30);
/// Warnings kept for the status, newest first.
const KEPT_WARNINGS: usize = 20;
/// Routes listed in the status (slowest p95 first).
const LISTED_ROUTES: usize = 15;

/// One warning, as logged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HealthWarning {
    /// ISO-8601 UTC.
    pub at: String,
    pub kind: WarningKind,
    pub message: String,
}

/// The process as last sampled, and its peak memory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProcessOut {
    pub rss_bytes: Option<u64>,
    pub peak_rss_bytes: Option<u64>,
    pub cpu_percent: Option<f32>,
}

/// `health` in `GET /api/status`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HealthOut {
    pub process: ProcessOut,
    pub writer: WriterStats,
    /// Slowest p95 first.
    pub routes: Vec<RouteOut>,
    /// The latest warnings, newest first.
    pub warnings: Vec<HealthWarning>,
}

/// Shared by the access log, the sampler and the status.
#[derive(Debug, Default)]
pub struct Health {
    routes: Mutex<RouteTable>,
    process: Mutex<ProcessOut>,
    warnings: Mutex<VecDeque<HealthWarning>>,
}

impl Health {
    /// Record one answered request.
    pub fn record(&self, method: &str, path: &str, status: u16, ms: f64) {
        lock(&self.routes).record(method, path, status, ms);
    }

    /// Requests answered 5xx and requests slower than the limit, so far.
    pub fn totals(&self) -> (u64, u64) {
        lock(&self.routes).totals()
    }

    pub fn set_process(&self, sample: ProcessSample) {
        let mut process = lock(&self.process);
        process.peak_rss_bytes = max_present(process.peak_rss_bytes, sample.rss_bytes);
        process.rss_bytes = sample.rss_bytes;
        process.cpu_percent = sample.cpu_percent;
    }

    /// Keep a warning for the status and log it.
    pub fn warn(&self, at: String, kind: WarningKind, message: String) {
        tracing::warn!(target: "gitprompt_server::health", ?kind, "{message}");
        let mut warnings = lock(&self.warnings);
        warnings.truncate(KEPT_WARNINGS - 1);
        warnings.push_front(HealthWarning { at, kind, message });
    }

    pub fn snapshot(&self, writer: WriterStats) -> HealthOut {
        HealthOut {
            process: *lock(&self.process),
            writer,
            routes: lock(&self.routes).slowest(LISTED_ROUTES),
            warnings: lock(&self.warnings).iter().cloned().collect(),
        }
    }
}

/// The guarded values stay consistent whatever panicked while holding them (plain updates).
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_are_kept_newest_first_and_bounded() {
        let health = Health::default();
        for i in 0..(KEPT_WARNINGS + 5) {
            health.warn(format!("t{i}"), WarningKind::Errors, format!("w{i}"));
        }
        let out = health.snapshot(WriterStats::default());
        assert_eq!(out.warnings.len(), KEPT_WARNINGS);
        assert_eq!(out.warnings[0].message, format!("w{}", KEPT_WARNINGS + 4));
    }

    #[test]
    fn the_peak_memory_outlives_a_lower_sample() {
        let health = Health::default();
        health.set_process(ProcessSample { rss_bytes: Some(300), cpu_percent: Some(5.0) });
        health.set_process(ProcessSample { rss_bytes: Some(100), cpu_percent: None });
        assert_eq!(
            health.snapshot(WriterStats::default()).process,
            ProcessOut { rss_bytes: Some(100), peak_rss_bytes: Some(300), cpu_percent: None }
        );
    }
}
