//! The results, as a table (default) or JSON (`--json`). Both phases share one read format.

use serde_json::{Value, json};

use gitprompt_store::{SLOW_HOLD, SLOW_WAIT};

use crate::args::Args;
use crate::bench::Ingest;
use crate::budget;
use crate::concurrent::Concurrent;
use crate::measure::{Read, ms};

/// Everything one run measured.
pub(crate) struct Results<'a> {
    pub(crate) ingest: &'a Ingest,
    pub(crate) db_bytes: u64,
    pub(crate) alone: &'a [Read],
    pub(crate) load: Option<&'a Concurrent>,
}

pub(crate) fn print(args: &Args, results: &Results<'_>) {
    if args.json {
        println!("{}", serde_json::to_string_pretty(&to_json(args, results)).unwrap_or_default());
    } else {
        print_table(args, results);
    }
}

fn print_table(args: &Args, r: &Results<'_>) {
    print_ingest(args, r.ingest, r.db_bytes);
    print_reads(r.alone);
    println!("\n{} timed runs per read after one cold run; via the axum router in-process (no sockets).", args.runs);
    if let Some(load) = r.load {
        print_load(args, load);
        print_reads(&load.reads);
    }
}

fn print_ingest(args: &Args, ingest: &Ingest, db_bytes: u64) {
    let secs = ingest.elapsed.as_secs_f64();
    let mb = ingest.bytes as f64 / 1e6;
    let events = ingest.events as f64;
    println!("Ingest  {} prompts x {} events/prompt = {} events", args.prompts, args.events_per_prompt + 2, ingest.events);
    println!(
        "        {secs:.1} s, {:.0} events/s, {:.1} MB/s ({mb:.1} MB of payloads), {:.2} ms/event",
        events / secs,
        mb / secs,
        ms(ingest.elapsed) / events
    );
    println!("Index   {:.1} ms (ensure after ingest)   database {:.1} MB", ms(ingest.index), db_bytes as f64 / 1e6);
}

fn print_load(args: &Args, load: &Concurrent) {
    let w = &load.writer;
    println!(
        "\nUnder load  {} s, {} payloads ingested, git {} ms; peak memory {} MiB, max CPU {:.0}%",
        args.concurrent.as_secs(),
        load.ingested,
        args.git_ms,
        load.peaks.rss_bytes.unwrap_or_default() >> 20,
        load.peaks.cpu_percent.unwrap_or_default()
    );
    println!(
        "            db writer: {} entries, {} waited > {} ms (max {} ms), {} kept > {} ms (max {} ms)",
        w.entries,
        w.slow_waits,
        SLOW_WAIT.as_millis(),
        w.max_wait_ms,
        w.slow_holds,
        SLOW_HOLD.as_millis(),
        w.max_hold_ms
    );
}

fn print_reads(reads: &[Read]) {
    println!();
    println!("{:<20} {:>8} {:>9} {:>9} {:>9} {:>9} {:>8}  path", "read", "requests", "first ms", "p50 ms", "p95 ms", "max ms", "KB");
    println!("{}", "-".repeat(104));
    for r in reads {
        println!(
            "{:<20} {:>8} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>8.1}  {}",
            r.name,
            r.samples.len(),
            ms(r.first),
            ms(r.quantile(0.5)),
            ms(r.quantile(0.95)),
            ms(r.max()),
            r.bytes as f64 / 1e3,
            r.path
        );
    }
}

fn to_json(args: &Args, r: &Results<'_>) -> Value {
    let ingest = r.ingest;
    let secs = ingest.elapsed.as_secs_f64();
    json!({
        "prompts": args.prompts,
        "events_per_prompt": args.events_per_prompt,
        "runs": args.runs,
        "ingest": {
            "events": ingest.events,
            "payload_bytes": ingest.bytes,
            "seconds": secs,
            "events_per_s": ingest.events as f64 / secs,
            "ms_per_event": ms(ingest.elapsed) / ingest.events as f64,
            "index_ms": ms(ingest.index),
        },
        "db_bytes": r.db_bytes,
        "reads": r.alone.iter().map(read_json).collect::<Vec<_>>(),
        "concurrent": r.load.map(|load| json!({
            "seconds": args.concurrent.as_secs(),
            "git_ms": args.git_ms,
            "ingested": load.ingested,
            "peaks": load.peaks,
            "writer": load.writer,
            "reads": load.reads.iter().map(read_json).collect::<Vec<_>>(),
        })),
        "over_budget": args.check.then(|| budget::exceeded(args, r.alone, r.load)),
    })
}

fn read_json(r: &Read) -> Value {
    json!({
        "name": r.name,
        "path": r.path,
        "status": r.status,
        "bytes": r.bytes,
        "requests": r.samples.len(),
        "first_ms": ms(r.first),
        "p50_ms": ms(r.quantile(0.5)),
        "p95_ms": ms(r.quantile(0.95)),
        "max_ms": ms(r.max()),
    })
}
