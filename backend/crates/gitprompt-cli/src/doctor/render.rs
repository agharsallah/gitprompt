//! The text form of a [`Report`]: one section per area, aligned labels, problems last.

use std::io::{self, Write};

use gitprompt_capture::connection::HookForm;
use gitprompt_core::spec::SpecOrigin;

use super::report::{ConnectorCheck, Finding, Report};

const WIDTH: usize = 14;

pub(super) fn text(report: &Report, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "GitPrompt doctor: version {} ({} backend)", report.version, report.backend)?;
    data(report, out)?;
    daemon(report, out)?;
    health(report, out)?;
    writeln!(out, "\nDashboard")?;
    line(out, "serve uses", &report.dashboard.describe())?;
    specs(report, out)?;
    connectors(&report.connectors, out)?;
    rest(report, out)?;
    findings(report, out)
}

fn line(out: &mut dyn Write, label: &str, value: &str) -> io::Result<()> {
    writeln!(out, "  {label:<WIDTH$} {value}")
}

fn yes_no(value: bool, yes: &str, no: &str) -> String {
    if value { yes.to_owned() } else { no.to_owned() }
}

fn data(report: &Report, out: &mut dyn Write) -> io::Result<()> {
    let d = &report.data;
    writeln!(out, "\nData")?;
    line(out, "data dir", &format!("{}{}", d.home, yes_no(d.home_exists, "", " (missing: created at first start)")))?;
    if !d.db_exists {
        return line(out, "database", &format!("{} (missing)", d.db_path));
    }
    line(out, "database", &format!("{} ({})", d.db_path, size(d.db_bytes)))?;
    line(out, "rows", &format!("{} prompts, {} events", d.prompts, d.events))?;
    if let Some(index) = &report.index {
        let state = if index.stale { "stale (rebuilt at the next read)" } else { "current" };
        line(out, "index", &format!("rules {state}, {} unindexed prompts, {} changed turns", index.unindexed, index.dirty_turns))?;
        line(out, "rules version", &format!("{} (current {})", index.rules_version.as_deref().unwrap_or("none"), index.current_version))?;
    }
    Ok(())
}

fn daemon(report: &Report, out: &mut dyn Write) -> io::Result<()> {
    let d = &report.daemon;
    writeln!(out, "\nDaemon")?;
    let listening = match (&d.running, d.healthz, d.listening) {
        (Some(r), _, _) => format!("GitPrompt {} ({}) answering, up {} s since {}", r.version, r.backend, r.uptime_s, r.started_at),
        (None, true, _) => "answers /healthz, but has no /api/status".to_owned(),
        (None, false, true) => "something else is listening".to_owned(),
        (None, false, false) => "nothing listening".to_owned(),
    };
    line(out, "port", &format!("{} ({listening})", d.port))?;
    line(out, "receiver", &format!("{} /healthz {}", d.receiver, yes_no(d.healthz, "ok", "unreachable")))?;
    if let Some(running) = &d.running {
        line(out, "serving", &format!("{}, dashboard {}", running.home, running.dashboard.describe()))?;
    }
    let lock = &d.lock;
    let pid = match (lock.pid, lock.pid_alive) {
        (Some(pid), Some(true)) => format!(", pid {pid} running"),
        (Some(pid), Some(false)) => format!(", pid {pid} not running"),
        (Some(pid), None) => format!(", pid {pid}"),
        (None, _) => String::new(),
    };
    line(out, "lock", &format!("{} ({}{pid})", lock.file, yes_no(lock.held, "held", "not held")))?;
    line(out, "token", &format!("{} ({})", d.token_file, yes_no(d.token_present, "present", "missing: created at first start")))
}

/// The running daemon's health: memory, CPU, the database writer, the slowest routes.
fn health(report: &Report, out: &mut dyn Write) -> io::Result<()> {
    let Some(running) = &report.daemon.running else { return Ok(()) };
    let h = &running.health;
    writeln!(out, "\nHealth (since the daemon started)")?;
    let p = &h.process;
    let memory = match (p.rss_bytes, p.peak_rss_bytes) {
        (Some(rss), Some(peak)) => format!("{} resident, peak {}", size(rss), size(peak)),
        _ => "not sampled yet".to_owned(),
    };
    line(out, "memory", &memory)?;
    line(out, "cpu", &p.cpu_percent.map_or_else(|| "not sampled yet".to_owned(), |cpu| format!("{cpu:.0}% of a core")))?;
    let w = &h.writer;
    line(
        out,
        "db writer",
        &format!(
            "{} writes, {} waited too long (max {} ms), {} kept it too long (max {} ms)",
            w.entries, w.slow_waits, w.max_wait_ms, w.slow_holds, w.max_hold_ms
        ),
    )?;
    for route in h.routes.iter().take(5) {
        line(
            out,
            "route",
            &format!(
                "{}: {} requests, p50 {} ms, p95 {} ms, max {} ms, {} slow, {} errors",
                route.route, route.count, route.p50_ms, route.p95_ms, route.max_ms, route.slow, route.errors
            ),
        )?;
    }
    for warning in h.warnings.iter().take(5) {
        line(out, "warning", &format!("{} {}", warning.at, warning.message))?;
    }
    Ok(())
}

fn specs(report: &Report, out: &mut dyn Write) -> io::Result<()> {
    let s = &report.specs;
    writeln!(out, "\nSpecs")?;
    let user: Vec<_> = s.files.iter().filter(|f| f.file.origin == SpecOrigin::User).collect();
    line(out, "loaded", &format!("{} sources ({} from user files in {})", s.files.len(), user.len(), s.user_dir))?;
    for f in user {
        line(out, "", &format!("{}: {}{}", f.id, f.file.path, yes_no(f.file.overrides, " (overrides the bundled spec)", "")))?;
    }
    line(out, "problems", &yes_no(s.problems.is_empty(), "none", &format!("{} (listed below)", s.problems.len())))
}

fn connectors(connectors: &[ConnectorCheck], out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "\nConnectors")?;
    for c in connectors {
        let install = match &c.install {
            None => String::new(),
            Some(HookForm::Curl) => ", curl hooks".to_owned(),
            Some(HookForm::Forwarder { path }) => {
                format!(", forwarder {}", path.as_deref().unwrap_or("(path unreadable)"))
            }
        };
        let flags = format!(
            "{}, {}{}{install}",
            yes_no(c.detected, "detected", "not detected"),
            yes_no(c.connected, "connected", "not connected"),
            yes_no(c.enabled, "", ", turned off"),
        );
        writeln!(out, "  {} ({}): {} [{flags}]", c.name, c.id, c.state_label)?;
        if c.connected || c.prompts > 0 || c.events > 0 {
            let last = c.last_at.as_deref().unwrap_or("never");
            writeln!(out, "  {:<WIDTH$} {} prompts, {} events, last {last}; {}", "", c.prompts, c.events, c.config_file)?;
        }
    }
    Ok(())
}

fn rest(report: &Report, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "\nSpool")?;
    if report.spool.is_empty() {
        line(out, "backlog", "empty")?;
    } else {
        let backlog: Vec<String> = report.spool.iter().map(|s| format!("{} {}", s.source, s.payloads)).collect();
        line(out, "backlog", &backlog.join(", "))?;
    }
    writeln!(out, "\nPreferences")?;
    if report.preferences.is_empty() {
        return line(out, "set", "none (all defaults)");
    }
    for p in &report.preferences {
        let from = serde_json::to_value(p.source).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        let from = if from == "env" { p.env.clone() } else { from };
        line(out, &p.key, &format!("{} (from {from})", p.value))?;
    }
    Ok(())
}

fn findings(report: &Report, out: &mut dyn Write) -> io::Result<()> {
    list(out, "Notes", &report.notes)?;
    list(out, "Problems", &report.problems)?;
    if report.problems.is_empty() {
        writeln!(out, "\nNo problems found.")?;
    }
    Ok(())
}

fn list(out: &mut dyn Write, title: &str, findings: &[Finding]) -> io::Result<()> {
    if findings.is_empty() {
        return Ok(());
    }
    writeln!(out, "\n{title} ({})", findings.len())?;
    for f in findings {
        writeln!(out, "  - {}\n    fix: {}", f.what, f.fix)?;
    }
    Ok(())
}

/// Bytes as `12.3 MB` / `456 KB` / `789 B`.
fn size(bytes: u64) -> String {
    match bytes {
        b if b >= 1_000_000 => format!("{:.1} MB", b as f64 / 1e6),
        b if b >= 1_000 => format!("{} KB", b / 1_000),
        b => format!("{b} B"),
    }
}
