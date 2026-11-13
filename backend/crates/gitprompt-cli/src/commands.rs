//! The commands, as `cli.py` runs them, plus the hook forwarder and the spool drain.

use std::fmt;
use std::time::Instant;

use jiff::Timestamp;

use gitprompt_capture::Connection;
use gitprompt_capture::forwarder::{Forwarder, HookRequest, Transport, Unreachable, build_request};
use gitprompt_capture::probe::detected;
use gitprompt_capture::spool::Spool;
use gitprompt_core::spec::{Spec, load_specs};
use gitprompt_core::{Json, Settings};
use gitprompt_insights::Specs;
use gitprompt_insights::captures::{self, RecentCapture};
use gitprompt_insights::indexer::Indexer;
use gitprompt_server::{AppOptions, Daemon, DaemonOptions, Dashboard};
use gitprompt_store::{Store, housekeeping};

use crate::Io;
use crate::http::HttpTransport;

/// Why a command failed: one line on stderr, exit 1.
pub(crate) struct Failure(String);

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<E: std::error::Error> From<E> for Failure {
    fn from(e: E) -> Self {
        Failure(e.to_string())
    }
}

type Outcome = Result<u8, Failure>;

/// `n` with thousands separators (Python's `{:,}`).
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Bytes as `{:,.1f} MB`.
fn mb(bytes: u64) -> String {
    // Tenths of a megabyte: a u64 byte count divided by 1e5 is non-negative and fits a u64.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "non-negative and at most u64::MAX / 1e5")]
    let tenths = (bytes as f64 / 1e5).round() as u64;
    format!("{}.{} MB", grouped(tenths / 10), tenths % 10)
}

/// Run the daemon until Ctrl-C. The dashboard is the folder named by `GITPROMPT_DASHBOARD`, else
/// the copy compiled into the binary.
pub(crate) fn serve(mut settings: Settings, port: Option<u16>, io: &mut Io<'_>) -> Outcome {
    if let Some(port) = port {
        settings.port = port;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(async {
        // Ctrl-C is caught from before the daemon answers: one that comes while it starts (it
        // answers before this function prints) stops it gracefully once it is up, instead of
        // killing it, which is what an uncaught SIGINT does.
        let interrupted = interrupt()?;
        let receiver = settings.receiver();
        let options = AppOptions { dashboard: Some(Dashboard::from_env(&settings.env)), ..AppOptions::default() };
        let daemon = Daemon::start(settings, options, DaemonOptions::default()).await?;
        writeln!(io.stdout, "GitPrompt is running on {receiver}")?;
        io.stdout.flush()?;
        interrupted.await;
        daemon.shutdown().await?;
        Ok(0)
    })
}

/// Ctrl-C (SIGINT), caught from now on; the future resolves at the first one. Inside the runtime.
#[cfg(unix)]
fn interrupt() -> std::io::Result<impl Future<Output = ()>> {
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    Ok(async move {
        sigint.recv().await;
    })
}

/// Ctrl-C; on Windows the handler is set when the future is first polled.
#[cfg(not(unix))]
fn interrupt() -> std::io::Result<impl Future<Output = ()>> {
    Ok(async {
        let _ = tokio::signal::ctrl_c().await;
    })
}

pub(crate) fn compact(settings: &Settings, apply: bool, older_than_days: Option<i64>, io: &mut Io<'_>) -> Outcome {
    let store = Store::open(&settings.db_path())?;
    let report = housekeeping::compact(&store, &load_specs(settings), older_than_days, Timestamp::now(), apply)?;
    for r in &report.removals {
        let why = serde_json::to_value(r.why)?;
        let why = why.as_str().unwrap_or_default();
        let event = r.event.as_deref().unwrap_or("None");
        writeln!(io.stdout, "{:>9}  {:>9}  {} {event}  ({why})", grouped(r.rows), mb(r.bytes), r.source)?;
    }
    writeln!(
        io.stdout,
        "{:>9}  {:>9}  events to remove, database is {}",
        grouped(report.rows()),
        mb(report.bytes()),
        mb(report.size_before)
    )?;
    match report.size_after {
        None => writeln!(io.stdout, "Dry run: nothing changed. Run again with --apply to delete these events and VACUUM.")?,
        Some(size) => writeln!(io.stdout, "Done: database is now {}.", mb(size))?,
    }
    Ok(0)
}

pub(crate) fn reindex(settings: &Settings, io: &mut Io<'_>) -> Outcome {
    let store = Store::open(&settings.db_path())?;
    let start = Instant::now();
    let count = Indexer::new(load_specs(settings)).rebuild(&store)?;
    // The per-event summary is a cache too (triggers keep it exact); a reindex repairs it as well.
    store.rebuild_event_stats()?;
    writeln!(io.stdout, "Indexed {} prompts in {:.1} s.", grouped(count as u64), start.elapsed().as_secs_f64())?;
    Ok(0)
}

/// `<n> <unit> ago`, the largest unit that fits.
fn ago(now: Timestamp, iso: &str) -> String {
    let seconds = iso.parse::<Timestamp>().map_or(0, |at| (now - at).get_seconds().max(0));
    for (unit, size) in [("d", 86_400), ("h", 3600), ("min", 60)] {
        if seconds >= size {
            return format!("{} {unit} ago", seconds / size);
        }
    }
    format!("{seconds} s ago")
}

pub(crate) fn check(settings: &Settings, io: &mut Io<'_>) -> Outcome {
    let specs = load_specs(settings);
    let captures = recent_captures(settings, &specs, io)?;
    let now = Timestamp::now();
    for tool in specs.values().filter(|s| s.part_of.is_none()) {
        writeln!(io.stdout, "\n{}", tool_status(tool, &specs, settings))?;
        let mine: Vec<String> = captures.iter().filter(|c| c.connector.id == tool.id).map(|c| capture_line(c, now)).collect();
        if mine.is_empty() {
            writeln!(io.stdout, "  no prompt captured yet")?;
        }
        for line in mine {
            writeln!(io.stdout, "  {line}")?;
        }
        for (app, why) in &tool.cannot_capture {
            writeln!(io.stdout, "  not capturable: {app}: {why}")?;
        }
    }
    Ok(0)
}

/// The latest capture of each tool, with the index brought up to date first; none (and a note
/// saying so) before the database exists.
fn recent_captures(settings: &Settings, specs: &Specs, io: &mut Io<'_>) -> Result<Vec<RecentCapture>, Failure> {
    let db = settings.db_path();
    if !db.exists() {
        writeln!(io.stdout, "No database yet at {}: start GitPrompt and send a prompt first.", db.display())?;
        return Ok(Vec::new());
    }
    let store = Store::open(&db)?;
    Indexer::new(specs.clone()).ensure(&store)?;
    Ok(captures::recent(&store, specs)?)
}

/// `<name>: <layer> connected, <layer> not connected…`, over the tool and the layers that are
/// `part_of` it, noting a tool not detected on this machine.
fn tool_status(tool: &Spec, specs: &Specs, settings: &Settings) -> String {
    let layers = std::iter::once(tool).chain(specs.values().map(AsRef::as_ref).filter(|s| s.part_of.as_deref() == Some(tool.id.as_str())));
    let state: Vec<String> = layers.map(|layer| layer_status(layer, settings)).collect();
    let found = if detected(tool, settings) { "" } else { " (not detected on this machine)" };
    format!("{}: {}{found}", tool.name, state.join(", "))
}

/// `<layer> connected` or `<layer> not connected`; a spec without a layer is the tool's hooks.
fn layer_status(spec: &Spec, settings: &Settings) -> String {
    let layer = if spec.layer.is_empty() { "hooks" } else { spec.layer.as_str() };
    let connected = if Connection::new(spec, settings).connected() { "connected" } else { "not connected" };
    format!("{layer} {connected}")
}

/// One capture: where, when, which folder and branch, session, model, prompts, then its signals.
fn capture_line(capture: &RecentCapture, now: Timestamp) -> String {
    let place = capture.surface.as_ref().map_or("surface unknown", |s| s.name.as_str());
    let project = capture.project.as_ref().map(|p| p.name.as_str()).filter(|n| !n.is_empty()).unwrap_or("no folder");
    let branch = capture.branch.as_ref().map(|b| format!(" ({b})")).unwrap_or_default();
    let session: String = capture.session.as_deref().unwrap_or("-").chars().take(12).collect();
    format!(
        "{place:<16} {:>9}  {project}{branch}  session {session}  {}  {} prompts{}",
        ago(now, &capture.received_at),
        capture.model.as_deref().unwrap_or("no model"),
        grouped(u64::try_from(capture.prompts).unwrap_or(0)),
        signals(capture),
    )
}

/// `  k=v k=v` (a string value unquoted), or nothing without signals.
fn signals(capture: &RecentCapture) -> String {
    let pairs: Vec<String> = capture
        .signals
        .iter()
        .map(|(key, value)| match value {
            Json::String(text) => format!("{key}={text}"),
            other => format!("{key}={other}"),
        })
        .collect();
    if pairs.is_empty() { String::new() } else { format!("  {}", pairs.join(" ")) }
}

/// Forward the payload on stdin; prints only the reply the tool expects; always 0.
pub(crate) fn hook(settings: &Settings, source: &str, io: &mut Io<'_>) -> u8 {
    let mut stdin = Vec::new();
    if io.stdin.read_to_end(&mut stdin).is_err() {
        return 0;
    }
    let Ok(token) = settings.token() else { return 0 };
    let specs = load_specs(settings);
    let forwarder = Forwarder { receiver: settings.receiver(), token, spool: Spool::of(settings), transport: &HttpTransport };
    let forwarded = match specs.get(source) {
        Some(spec) => forwarder.forward_hook(spec, &stdin),
        None => forwarder.forward(source, &stdin, None),
    };
    // A tool that can't read the reply has nothing more to learn from us.
    let _ = io.stdout.write_all(forwarded.stdout.as_bytes()).and_then(|()| io.stdout.flush());
    0
}

/// Deliver every spooled payload to the running daemon, oldest first.
pub(crate) fn drain(settings: &Settings, io: &mut Io<'_>) -> Outcome {
    let spool = Spool::of(settings);
    let token = settings.token()?;
    let receiver = settings.receiver();
    let mut delivered = 0;
    let mut down = false;
    for source in spool.sources()? {
        let mut send = |payload: Json| -> gitprompt_core::Result<()> {
            let request = build_request(&receiver, &source, &token, &serde_json::to_vec(&payload)?);
            deliver(&HttpTransport, &request, &source).map_err(|unreachable| {
                down = true;
                gitprompt_core::Error::Conflict(unreachable.to_string())
            })
        };
        delivered += spool.drain(&source, &mut send)?.ingested;
        if down {
            break;
        }
    }
    let left = spool.sources()?.iter().map(|source| spool.pending(source).map(|pending| pending.len())).sum::<Result<usize, _>>()?;
    if down {
        return Err(Failure(format!("GitPrompt is not running on {receiver}: {left} spooled payloads left.")));
    }
    writeln!(io.stdout, "Delivered {delivered} spooled payloads, {left} left.")?;
    Ok(0)
}

/// Send one spooled payload. A refusal (any non-2xx answer) drops it with a warning; only a
/// daemon that cannot be reached is an error, which keeps the payload spooled.
fn deliver(transport: &impl Transport, request: &HookRequest, source: &str) -> Result<(), Unreachable> {
    let status = transport.send(request)?;
    if !(200..300).contains(&status) {
        tracing::warn!(%source, status, "the receiver refused a spooled payload; dropped");
    }
    Ok(())
}
