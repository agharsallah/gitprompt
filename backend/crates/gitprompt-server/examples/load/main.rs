//! Load benchmark: ingest throughput and dashboard read latencies on a realistically sized store.
//!
//! ```text
//! cargo run --release -p gitprompt-server --example load -- \
//!     [--prompts N] [--events-per-prompt M] [--runs R] [--home DIR] [--json]
//! ```
//!
//! 1. Builds an [`App`] on a temp data directory (or `--home DIR`, which must be empty or a store
//!    this benchmark filled before; the real `~/.gitprompt` is never used unless you pass it).
//! 2. Ingests `N` synthetic Claude Code turns — `UserPromptSubmit`, `M` × `PostToolUse` with
//!    realistic payload sizes (file reads, edits, shell output), `Stop` with the answer — through
//!    [`gitprompt_server::Service::ingest`], the path HTTP ingest takes after its auth check, one
//!    payload at a time as hooks deliver them. Reports events/s and MB/s.
//! 3. Brings the prompt index up to date (what the daemon does at start) and times it.
//! 4. Times the main dashboard reads through the axum router (`tower::ServiceExt::oneshot`, no
//!    sockets): the first call (cold caches) and p50 / p95 / max over `R` more runs.
//!
//! 5. With `--concurrent S` (or `--check`): for `S` seconds, two hook streams ingest while readers
//!    hit the prompt list, a prompt, stats and connectors, as a busy session with the dashboard
//!    open does. Git answers in `--git-ms` (a cold cache or a big repository) in this phase only.
//!    Reports read latencies under load, how the database writer was shared, peak resident memory
//!    and CPU.
//! 6. With `--check`: exits 1 when a budget is exceeded — any read p95 over `--max-read-p95-ms`, the
//!    database writer kept over `gitprompt_store::SLOW_HOLD` during the concurrent phase (slow work
//!    inside a transaction), peak memory over `--max-rss-mb`, or no ingest progress. CI runs it.
//!
//! Git and tool versions are pinned (no subprocess per ingest), so numbers measure GitPrompt only.
//!
//! Files: [`args`], [`payloads`] (synthetic turns), [`bench`] (the app and its reads), [`measure`],
//! [`concurrent`] (step 5), [`budget`] (step 6), [`report`].
#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::format_push_string,
    reason = "benchmark binary: prints its report to stdout and aborts on any setup failure"
)]

mod args;
mod bench;
mod budget;
mod concurrent;
mod measure;
mod payloads;
mod report;

use std::process::ExitCode;

use args::{Args, USAGE, parse_args};
use bench::Bench;
use measure::time_read;
use report::Results;

async fn run(args: &Args) -> Result<(), String> {
    let bench = Bench::open(args)?;
    progress(
        args,
        &format!("Ingesting {} turns x ({} + 2) events into {} ...", args.prompts, args.events_per_prompt, bench.home.display()),
    );
    let ingest = bench.fill(args).await?;

    let specs = bench.reads().await?;
    let mut alone = Vec::with_capacity(specs.len());
    for spec in &specs {
        alone.push(time_read(&bench.router, &bench.host, spec.name, &spec.path, args.runs).await?);
    }

    let load = if args.concurrent.is_zero() {
        None
    } else {
        progress(
            args,
            &format!("Ingesting and reading at once for {} s (git answers in {} ms) ...", args.concurrent.as_secs(), args.git_ms),
        );
        Some(concurrent::run(args, &bench, &specs).await?)
    };

    report::print(args, &Results { ingest: &ingest, db_bytes: bench.db_bytes(), alone: &alone, load: load.as_ref() });
    if args.check {
        let over = budget::exceeded(args, &alone, load.as_ref());
        if !over.is_empty() {
            return Err(format!("over budget:\n  - {}", over.join("\n  - ")));
        }
        eprintln!("load: every budget met");
    }
    Ok(())
}

/// A progress line on stderr, except for `--json` (stdout stays machine-readable either way).
fn progress(args: &Args, line: &str) {
    if !args.json {
        eprintln!("{line}");
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(why) => {
            if !why.is_empty() {
                eprintln!("load: {why}");
            }
            eprintln!("{USAGE}");
            return if why.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(2) };
        }
    };
    match run(&args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("load: {why}");
            ExitCode::FAILURE
        }
    }
}
