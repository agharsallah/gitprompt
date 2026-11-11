//! Timing reads through the axum router in-process (no sockets).

use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;

use gitprompt_server::health::percentile;

/// The timings of one read.
pub(crate) struct Read {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) status: u16,
    pub(crate) bytes: usize,
    pub(crate) first: Duration,
    pub(crate) samples: Vec<Duration>,
}

impl Read {
    /// Nearest-rank quantile of the samples; zero when there are none.
    pub(crate) fn quantile(&self, q: f64) -> Duration {
        let mut sorted = self.samples.clone();
        sorted.sort();
        percentile(&sorted, q)
    }

    pub(crate) fn max(&self) -> Duration {
        self.samples.iter().copied().max().unwrap_or_default()
    }
}

/// `GET path`: the status and the body.
pub(crate) async fn get(router: &Router, host: &str, path: &str) -> Result<(u16, Vec<u8>), String> {
    let request = Request::get(path).header("host", host).body(Body::empty()).map_err(|e| e.to_string())?;
    let response = router.clone().oneshot(request).await.map_err(|e| format!("{path}: {e}"))?;
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.map_err(|e| format!("{path}: {e}"))?.to_bytes().to_vec();
    Ok((status, body))
}

/// One cold call, then `runs` timed ones; an error when the read does not answer 200.
pub(crate) async fn time_read(router: &Router, host: &str, name: &str, path: &str, runs: usize) -> Result<Read, String> {
    let started = Instant::now();
    let (status, body) = get(router, host, path).await?;
    let first = started.elapsed();
    if status != 200 {
        return Err(format!("{name} ({path}): {status} {}", String::from_utf8_lossy(&body)));
    }
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let started = Instant::now();
        get(router, host, path).await?;
        samples.push(started.elapsed());
    }
    Ok(Read { name: name.to_owned(), path: path.to_owned(), status, bytes: body.len(), first, samples })
}

pub(crate) fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
