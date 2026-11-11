//! The concurrent phase: hook streams ingest while the dashboard reads, with git slowed down, as a
//! busy session with the dashboard open does.

use std::time::{Duration, Instant};

use axum::Router;
use tokio::task::JoinHandle;

use gitprompt_server::Service;
use gitprompt_server::health::{Peaks, ProcessSampler};
use gitprompt_store::WriterStats;

use crate::args::Args;
use crate::bench::{Bench, ReadSpec};
use crate::measure::{Read, get};
use crate::payloads::{Rng, turn_payloads};

/// Hook streams ingesting at once.
const STREAMS: usize = 2;
/// Pause between two reads of one reader: a busy dashboard, not a flood.
const READ_PAUSE: Duration = Duration::from_millis(20);
const SAMPLE_PAUSE: Duration = Duration::from_millis(250);

pub(crate) struct Concurrent {
    pub(crate) ingested: usize,
    pub(crate) reads: Vec<Read>,
    /// How the writer was shared during this phase only.
    pub(crate) writer: WriterStats,
    pub(crate) peaks: Peaks,
}

/// Run the phase for `--concurrent` seconds on the reads marked `under_load`.
pub(crate) async fn run(args: &Args, bench: &Bench, reads: &[ReadSpec]) -> Result<Concurrent, String> {
    let before = writer_stats(&bench.service).await?;
    bench.git.set_delay(Duration::from_millis(args.git_ms));
    let deadline = Instant::now() + args.concurrent;

    let ingesters: Vec<_> = (0..STREAMS).map(|stream| ingester(bench.service.clone(), args.prompts, stream, deadline)).collect();
    let readers: Vec<_> = reads.iter().filter(|r| r.under_load).map(|r| reader(bench, r, deadline)).collect();
    let sampler = sampler(deadline);

    let ingested = join_all(ingesters).await?.into_iter().sum();
    let reads = join_all(readers).await?;
    let peaks = sampler.await.map_err(|e| e.to_string())?;
    bench.git.set_delay(Duration::ZERO);
    let writer = writer_stats(&bench.service).await?.since(before);
    Ok(Concurrent { ingested, reads, writer, peaks })
}

/// One hook stream: new turns (never deduplicated) until `deadline`; the payloads stored.
fn ingester(service: Service, filled: usize, stream: usize, deadline: Instant) -> JoinHandle<Result<usize, String>> {
    tokio::spawn(async move {
        let mut rng = Rng(0xD1B5_4A32_D192_ED03 ^ stream as u64);
        let first = filled + (stream + 1) * 1_000_000;
        let mut stored = 0;
        for turn in first.. {
            for payload in turn_payloads(&mut rng, turn, 3) {
                if Instant::now() >= deadline {
                    return Ok(stored);
                }
                service.ingest("claude-code", payload).await.map_err(|e| format!("ingest: {e}"))?;
                stored += 1;
            }
        }
        Ok(stored)
    })
}

/// One dashboard read, repeated until `deadline`.
fn reader(bench: &Bench, spec: &ReadSpec, deadline: Instant) -> JoinHandle<Result<Read, String>> {
    let (router, host): (Router, String) = (bench.router.clone(), bench.host.clone());
    let (name, path) = (spec.name.to_owned(), spec.path.clone());
    tokio::spawn(async move {
        let (mut samples, mut status, mut bytes) = (Vec::new(), 200, 0);
        while Instant::now() < deadline {
            let started = Instant::now();
            let (code, body) = get(&router, &host, &path).await?;
            samples.push(started.elapsed());
            (status, bytes) = (code, body.len());
            tokio::time::sleep(READ_PAUSE).await;
        }
        let first = samples.first().copied().unwrap_or_default();
        Ok(Read { name, path, status, bytes, first, samples })
    })
}

/// Peak memory and CPU of this process until `deadline`.
fn sampler(deadline: Instant) -> JoinHandle<Peaks> {
    tokio::spawn(async move {
        let (mut sampler, mut peaks) = (ProcessSampler::default(), Peaks::default());
        while Instant::now() < deadline {
            peaks.add(sampler.sample());
            tokio::time::sleep(SAMPLE_PAUSE).await;
        }
        peaks
    })
}

async fn join_all<T>(tasks: Vec<JoinHandle<Result<T, String>>>) -> Result<Vec<T>, String> {
    let mut out = Vec::with_capacity(tasks.len());
    for task in tasks {
        out.push(task.await.map_err(|e| e.to_string())??);
    }
    Ok(out)
}

async fn writer_stats(service: &Service) -> Result<WriterStats, String> {
    Ok(service.status().await.map_err(|e| format!("status: {e}"))?.health.writer)
}
