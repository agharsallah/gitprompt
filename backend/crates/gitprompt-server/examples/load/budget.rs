//! `--check`: the budgets a run must meet, each exceeded one in words.

use gitprompt_store::SLOW_HOLD;

use crate::args::Args;
use crate::concurrent::Concurrent;
use crate::measure::{Read, ms};

/// Every budget exceeded (empty: all met).
pub(crate) fn exceeded(args: &Args, alone: &[Read], load: Option<&Concurrent>) -> Vec<String> {
    let mut over: Vec<String> = alone.iter().flat_map(|read| read_over(args, "alone", read)).collect();
    if let Some(load) = load {
        over.extend(load.reads.iter().flat_map(|read| read_over(args, "under load", read)));
        over.extend(load_over(args, load));
    }
    over
}

/// A read must answer 200 within the p95 budget.
fn read_over(args: &Args, phase: &str, read: &Read) -> Vec<String> {
    if read.samples.is_empty() {
        return vec![format!("{} ({phase}): never answered", read.name)];
    }
    let p95 = ms(read.quantile(0.95));
    let mut over = Vec::new();
    if p95 > args.max_read_p95_ms {
        over.push(format!("{} ({phase}): p95 {p95:.1} ms > {} ms", read.name, args.max_read_p95_ms));
    }
    if read.status != 200 {
        over.push(format!("{} ({phase}): status {}", read.name, read.status));
    }
    over
}

/// Under load: no slow work inside a transaction, ingest progress, bounded memory.
fn load_over(args: &Args, load: &Concurrent) -> Vec<String> {
    let mut over = Vec::new();
    let writer = &load.writer;
    if writer.slow_holds > 0 {
        over.push(format!(
            "the database writer was kept over {} ms {} times under load (longest {} ms): slow work inside a transaction; the `held the database writer` warnings name where",
            SLOW_HOLD.as_millis(),
            writer.slow_holds,
            writer.max_hold_ms
        ));
    }
    if load.ingested == 0 {
        over.push("no payload was ingested under load".to_owned());
    }
    if let Some(peak) = load.peaks.rss_bytes.filter(|peak| *peak > args.max_rss_mb << 20) {
        over.push(format!("peak resident memory {} MiB > {} MiB", peak >> 20, args.max_rss_mb));
    }
    over
}
