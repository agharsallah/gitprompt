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
//! Git and tool versions are pinned (no subprocess per ingest), so numbers measure GitPrompt only.
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

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use gitprompt_core::Settings;
use gitprompt_server::{App, AppOptions, FixedVersion, NoGit};

const USAGE: &str = "usage: load [--prompts N] [--events-per-prompt M] [--runs R] [--home DIR] [--json]

  --prompts N            turns to ingest (default 2000)
  --events-per-prompt M  PostToolUse events per turn (default 20)
  --runs R               timed runs per read, after one cold run (default 20)
  --home DIR             data directory to fill (default: a fresh temp directory)
  --json                 print machine-readable results instead of a table";

struct Args {
    prompts: usize,
    events_per_prompt: usize,
    runs: usize,
    home: Option<PathBuf>,
    json: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { prompts: 2000, events_per_prompt: 20, runs: 20, home: None, json: false };
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        let mut value = |name: &str| argv.next().ok_or_else(|| format!("{name} needs a value"));
        let mut count = |name: &str| -> Result<usize, String> {
            let text = value(name)?;
            text.parse().map_err(|_| format!("{name}: {text:?} is not a whole number"))
        };
        match flag.as_str() {
            "--prompts" => args.prompts = count("--prompts")?,
            "--events-per-prompt" => args.events_per_prompt = count("--events-per-prompt")?,
            "--runs" => args.runs = count("--runs")?.max(1),
            "--home" => args.home = Some(PathBuf::from(value("--home")?)),
            "--json" => args.json = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if args.prompts == 0 {
        return Err("--prompts must be at least 1".into());
    }
    Ok(args)
}

// --- synthetic payloads (Claude Code hook shapes, https://code.claude.com/docs/en/hooks) ---

/// A small deterministic generator (xorshift64*), so every run ingests the same data.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const WORDS: &[&str] = &[
    "add",
    "retry",
    "upload",
    "client",
    "refactor",
    "parser",
    "test",
    "cache",
    "invalidate",
    "error",
    "handling",
    "config",
    "migrate",
    "schema",
    "endpoint",
    "pagination",
    "timeout",
    "logging",
    "flaky",
    "fixture",
    "render",
    "chart",
    "session",
    "token",
    "rotate",
];
const TOOLS: &[&str] = &["Read", "Edit", "Bash", "Grep", "Write", "Glob"];
const PROJECTS: usize = 8;
const PROMPTS_PER_SESSION: usize = 25;

/// `min` to `min + spread` bytes of words.
fn text(rng: &mut Rng, min: usize, spread: usize) -> String {
    let bytes = min + rng.below(spread);
    let mut out = String::with_capacity(bytes + 16);
    while out.len() < bytes {
        if !out.is_empty() {
            out.push(if rng.below(12) == 0 { '\n' } else { ' ' });
        }
        out.push_str(rng.pick(WORDS));
    }
    out
}

/// `min` to `min + spread` bytes of source-looking lines.
fn code(rng: &mut Rng, min: usize, spread: usize) -> String {
    let bytes = min + rng.below(spread);
    let mut out = String::with_capacity(bytes + 64);
    let mut line = 1;
    while out.len() < bytes {
        let (a, b) = (rng.pick(WORDS), rng.pick(WORDS));
        out.push_str(&format!("{line:>5}\tfn {a}_{b}(input: &str) -> Result<(), Error> {{ {a}(input)?; Ok(()) }}\n"));
        line += 1;
    }
    out
}

struct Turn {
    session: String,
    prompt_id: String,
    cwd: String,
}

fn base(turn: &Turn, event: &str) -> Value {
    json!({
        "session_id": turn.session,
        "prompt_id": turn.prompt_id,
        "transcript_path": format!("/home/dev/.claude/projects/app/{}.jsonl", turn.session),
        "cwd": turn.cwd,
        "permission_mode": "default",
        "hook_event_name": event,
    })
}

fn merge(mut payload: Value, extra: Value) -> Value {
    if let (Some(target), Value::Object(extra)) = (payload.as_object_mut(), extra) {
        target.extend(extra);
    }
    payload
}

/// One `PostToolUse`, 0.3–6 KB depending on the tool.
fn tool_use(rng: &mut Rng, turn: &Turn, n: usize) -> Value {
    let tool = rng.pick(TOOLS);
    let file = format!("{}/src/{}_{}.rs", turn.cwd, rng.pick(WORDS), rng.pick(WORDS));
    let (input, response) = match tool {
        "Read" => (json!({"file_path": file}), json!({"type": "text", "file": {"filePath": file, "content": code(rng, 1500, 4500)}})),
        "Edit" => {
            let (old, new) = (code(rng, 200, 800), code(rng, 200, 800));
            (json!({"file_path": file, "old_string": old, "new_string": new}), json!({"filePath": file, "success": true}))
        }
        "Write" => (json!({"file_path": file, "content": code(rng, 800, 2500)}), json!({"filePath": file, "success": true})),
        "Bash" => (
            json!({"command": format!("cargo test -p {} --quiet", rng.pick(WORDS)), "description": "Run the tests"}),
            json!({"stdout": text(rng, 300, 3000), "stderr": "", "interrupted": false}),
        ),
        "Grep" => (json!({"pattern": rng.pick(WORDS), "path": turn.cwd}), json!({"mode": "content", "content": code(rng, 200, 1500)})),
        _ => {
            (json!({"pattern": "**/*.rs"}), json!({"filenames": (0..20).map(|i| format!("{}/src/f{i}.rs", turn.cwd)).collect::<Vec<_>>()}))
        }
    };
    merge(
        base(turn, "PostToolUse"),
        json!({"tool_name": tool, "tool_input": input, "tool_response": response, "tool_use_id": format!("{}-t{n}", turn.prompt_id)}),
    )
}

/// The payloads of turn `i`: prompt, `events` tool uses, stop.
fn turn_payloads(rng: &mut Rng, i: usize, events: usize) -> Vec<Value> {
    let session_no = i / PROMPTS_PER_SESSION;
    let turn = Turn {
        session: format!("bench-session-{session_no:05}"),
        prompt_id: format!("bench-prompt-{i:07}"),
        cwd: format!("/work/project-{}", session_no % PROJECTS),
    };
    let mut out = Vec::with_capacity(events + 2);
    let prompt = text(rng, 80, 1200);
    out.push(merge(base(&turn, "UserPromptSubmit"), json!({"prompt": prompt})));
    out.extend((0..events).map(|n| tool_use(rng, &turn, n)));
    let answer = text(rng, 200, 1500);
    out.push(merge(base(&turn, "Stop"), json!({"stop_hook_active": false, "last_assistant_message": answer})));
    out
}

// --- measuring ---

struct Ingest {
    events: usize,
    bytes: usize,
    elapsed: Duration,
    index: Duration,
}

struct Read {
    name: String,
    path: String,
    status: u16,
    bytes: usize,
    first: Duration,
    samples: Vec<Duration>,
}

impl Read {
    fn quantile(&self, q: f64) -> Duration {
        let mut sorted = self.samples.clone();
        sorted.sort();
        let rank = ((sorted.len() as f64 * q).ceil() as usize).clamp(1, sorted.len());
        sorted[rank - 1]
    }

    fn max(&self) -> Duration {
        self.samples.iter().copied().max().unwrap_or_default()
    }
}

async fn get(router: &Router, host: &str, path: &str) -> Result<(u16, Vec<u8>), String> {
    let request = Request::get(path).header("host", host).body(Body::empty()).map_err(|e| e.to_string())?;
    let response = router.clone().oneshot(request).await.map_err(|e| format!("{path}: {e}"))?;
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.map_err(|e| format!("{path}: {e}"))?.to_bytes().to_vec();
    Ok((status, body))
}

async fn time_read(router: &Router, host: &str, name: &str, path: &str, runs: usize) -> Result<Read, String> {
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

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

async fn run(args: &Args) -> Result<(), String> {
    let _tmp;
    let home = if let Some(dir) = &args.home {
        dir.clone()
    } else {
        let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let dir = tmp.path().join("gitprompt");
        _tmp = tmp;
        dir
    };
    let user_home = home.parent().map_or_else(|| home.join("home"), |p| p.join("bench-home"));
    std::fs::create_dir_all(&user_home).map_err(|e| format!("{}: {e}", user_home.display()))?;
    let settings = Settings::new(&home, &user_home);
    let host = format!("{}:{}", settings.host(), settings.port);
    let options = AppOptions { tool_version: Arc::new(FixedVersion(Some("2.1.0".into()))), git: Arc::new(NoGit), ..AppOptions::default() };
    let app = App::new(settings, options).map_err(|e| e.to_string())?;
    let service = app.service();

    if !args.json {
        eprintln!("Ingesting {} turns x ({} + 2) events into {} ...", args.prompts, args.events_per_prompt, home.display());
    }
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut events, mut bytes, mut elapsed) = (0, 0, Duration::ZERO);
    for i in 0..args.prompts {
        for payload in turn_payloads(&mut rng, i, args.events_per_prompt) {
            bytes += serde_json::to_vec(&payload).map_err(|e| e.to_string())?.len(); // what a hook sends
            let started = Instant::now();
            service.ingest("claude-code", payload).await.map_err(|e| format!("ingest: {e}"))?;
            elapsed += started.elapsed();
            events += 1;
        }
    }
    let started = Instant::now();
    service.ensure_index().await.map_err(|e| format!("index: {e}"))?;
    let ingest = Ingest { events, bytes, elapsed, index: started.elapsed() };

    let router = app.router();
    let (_, list) = get(&router, &host, "/api/prompts?limit=50").await?;
    let list: Value = serde_json::from_slice(&list).map_err(|e| format!("/api/prompts: {e}"))?;
    let sample = list.as_array().and_then(|l| l.get(l.len() / 2)).ok_or("no prompt listed")?;
    let id = sample["id"].as_i64().ok_or("a prompt without id")?;
    let session = sample["fields"]["session_id"].as_str().ok_or("a prompt without session")?.to_owned();
    let reads = [
        ("prompts (limit=50)", "/api/prompts?limit=50".to_owned()),
        ("prompt detail", format!("/api/prompts/{id}")),
        ("stats by day", "/api/stats?group_by=day".to_owned()),
        ("stats by model", "/api/stats?group_by=model".to_owned()),
        ("stats by tool", "/api/stats?group_by=tool".to_owned()),
        ("sources", "/api/sources".to_owned()),
        ("data", "/api/data".to_owned()),
        ("session story", format!("/api/stories/session/{session}")),
        ("search", "/api/search?q=retry".to_owned()),
        ("fields", "/api/fields".to_owned()),
    ];
    let mut results = Vec::with_capacity(reads.len());
    for (name, path) in &reads {
        results.push(time_read(&router, &host, name, path, args.runs).await?);
    }
    let db_bytes = std::fs::metadata(home.join("gitprompt.db")).map(|m| m.len()).unwrap_or_default();

    if args.json {
        print_json(args, &ingest, &results, db_bytes);
    } else {
        print_table(args, &ingest, &results, db_bytes);
    }
    Ok(())
}

fn print_table(args: &Args, ingest: &Ingest, reads: &[Read], db_bytes: u64) {
    let secs = ingest.elapsed.as_secs_f64();
    println!("Ingest  {} prompts x {} events/prompt = {} events", args.prompts, args.events_per_prompt + 2, ingest.events);
    println!(
        "        {:.1} s, {:.0} events/s, {:.1} MB/s ({:.1} MB of payloads), {:.2} ms/event",
        secs,
        ingest.events as f64 / secs,
        ingest.bytes as f64 / 1e6 / secs,
        ingest.bytes as f64 / 1e6,
        ms(ingest.elapsed) / ingest.events as f64
    );
    println!("Index   {:.1} ms (ensure after ingest)   database {:.1} MB", ms(ingest.index), db_bytes as f64 / 1e6);
    println!();
    println!("{:<20} {:>10} {:>10} {:>10} {:>10} {:>10}  path", "read", "first ms", "p50 ms", "p95 ms", "max ms", "KB");
    println!("{}", "-".repeat(100));
    for r in reads {
        println!(
            "{:<20} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>10.1}  {}",
            r.name,
            ms(r.first),
            ms(r.quantile(0.5)),
            ms(r.quantile(0.95)),
            ms(r.max()),
            r.bytes as f64 / 1e3,
            r.path
        );
    }
    println!("\n{} timed runs per read after one cold run; via the axum router in-process (no sockets).", args.runs);
}

fn print_json(args: &Args, ingest: &Ingest, reads: &[Read], db_bytes: u64) {
    let secs = ingest.elapsed.as_secs_f64();
    let out = json!({
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
        "db_bytes": db_bytes,
        "reads": reads.iter().map(|r| json!({
            "name": r.name,
            "path": r.path,
            "status": r.status,
            "bytes": r.bytes,
            "first_ms": ms(r.first),
            "p50_ms": ms(r.quantile(0.5)),
            "p95_ms": ms(r.quantile(0.95)),
            "max_ms": ms(r.max()),
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
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
