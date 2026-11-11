//! From the facts of a [`Report`] to what needs attention (problems, exit 1) and what is worth
//! knowing (notes), each with a one-line fix.

use gitprompt_capture::connection::{ConnectorState, HookForm};
use gitprompt_server::DashboardSource;
use gitprompt_server::access_log::is_slow;
use gitprompt_server::health::WarningKind;
use gitprompt_server::status::BACKEND;

use super::report::{Finding, Report};

/// Problems and notes, in the order of the report's sections.
pub(super) fn derive(report: &Report) -> (Vec<Finding>, Vec<Finding>) {
    let mut found = Found::default();
    data(report, &mut found);
    daemon(report, &mut found);
    health(report, &mut found);
    dashboard(report, &mut found);
    specs(report, &mut found);
    connectors(report, &mut found);
    spool(report, &mut found);
    (found.problems, found.notes)
}

#[derive(Default)]
struct Found {
    problems: Vec<Finding>,
    notes: Vec<Finding>,
}

impl Found {
    fn problem(&mut self, what: impl Into<String>, fix: impl Into<String>) {
        self.problems.push(Finding::new(what, fix));
    }
    fn note(&mut self, what: impl Into<String>, fix: impl Into<String>) {
        self.notes.push(Finding::new(what, fix));
    }
}

fn data(report: &Report, found: &mut Found) {
    if !report.data.db_exists {
        found.note("No database yet.", "It is created when GitPrompt starts (`gitprompt serve` or the desktop app).");
    }
    if let Some(index) = &report.index
        && index.stale
        && report.data.prompts > 0
    {
        found.note(
            "The prompt index was built with other rules (specs, time zone or version changed).",
            "It is rebuilt at the next dashboard read; `gitprompt reindex` does it now.",
        );
    }
}

fn daemon(report: &Report, found: &mut Found) {
    let d = &report.daemon;
    let port = d.port;
    let connected = report.connectors.iter().any(|c| c.connected);
    match (&d.running, d.healthz, d.listening) {
        (Some(running), _, _) => {
            if running.home != report.data.home {
                found.problem(
                    format!("The GitPrompt on port {port} serves another data directory ({}).", running.home),
                    "Use the same GITPROMPT_HOME for the daemon and this command, or another GITPROMPT_PORT.",
                );
            }
            if running.backend != BACKEND {
                found.problem(
                    format!("The daemon on port {port} is the {} backend.", running.backend),
                    "Stop it and start `gitprompt serve`.",
                );
            }
        }
        (None, true, _) => found.problem(
            format!("Port {port} answers /healthz but not /api/status: the deprecated Python daemon?"),
            "Stop it and start `gitprompt serve` or the desktop app.",
        ),
        (None, false, true) => found.problem(
            format!("Port {port} is used by a program that is not GitPrompt."),
            "Stop that program, or set GITPROMPT_PORT to a free port (then reconnect the connectors).",
        ),
        (None, false, false) if d.lock.held => {}
        (None, false, false) if connected => found.problem(
            "GitPrompt is not running: connected hooks can't deliver (curl hooks lose prompts, the forwarder spools them).",
            "Start it: `gitprompt serve` or the desktop app.",
        ),
        (None, false, false) => found.note("GitPrompt is not running.", "Start it: `gitprompt serve` or the desktop app."),
    }
    let lock = &d.lock;
    if lock.held {
        match (lock.pid, lock.pid_alive) {
            (Some(pid), Some(false)) => found.problem(
                format!("The daemon lock {} is held, but pid {pid} written in it is not running.", lock.file),
                format!("A process that inherited the lock holds it: find it with `lsof {}` and quit it.", lock.file),
            ),
            (Some(pid), _) if !d.listening => found.problem(
                format!("GitPrompt (pid {pid}) holds the lock but nothing answers on port {port}."),
                "It may still be starting or listen on another GITPROMPT_PORT; otherwise quit and restart it.",
            ),
            _ => {}
        }
    }
    if !d.token_present && connected {
        found.problem(
            format!("No receiver token ({}), but connectors are connected.", d.token_file),
            "Start GitPrompt (it creates one), then rotate the token in Settings > Security so the hooks carry it.",
        );
    }
}

/// What the running daemon's health monitor saw: memory and server errors are problems, slowness
/// is worth knowing.
fn health(report: &Report, found: &mut Found) {
    let Some(running) = &report.daemon.running else { return };
    let h = &running.health;
    let mut seen = Vec::new();
    for warning in &h.warnings {
        if seen.contains(&warning.kind) {
            continue; // newest first: one finding per kind
        }
        seen.push(warning.kind);
        let what = format!("{} (at {}).", warning.message, warning.at);
        match warning.kind {
            WarningKind::MemoryHigh | WarningKind::MemoryGrowth => {
                found.problem(what, "Restart GitPrompt to recover the memory, and report it with this output and the daemon log.");
            }
            WarningKind::Errors => found.problem(what, "The daemon log has each failure (`request failed`); report it if it repeats."),
            WarningKind::CpuHigh => found.note(what, "Usually a large import, reindex or busy session; restart GitPrompt if it lasts."),
            WarningKind::WriterContention => {
                found.note(what, "The daemon log names the code that kept the database writer (`held the database writer ... at=`).");
            }
            WarningKind::SlowRequests => found.note(what, "The routes below show which; the daemon log has each `slow request`."),
        }
    }
    for route in h.routes.iter().filter(|r| is_slow(r.p95_ms)) {
        found.note(
            format!("{} is slow: p95 {} ms over its recent requests (max {} ms).", route.route, route.p95_ms, route.max_ms),
            "Check the db writer line above; `just perf` (in a source checkout) reproduces this kind of load.",
        );
    }
}

fn dashboard(report: &Report, found: &mut Found) {
    let info = &report.dashboard;
    match (info.source, info.built) {
        (DashboardSource::Dir, false) => found.problem(
            format!("GITPROMPT_DASHBOARD names {}, which has no built dashboard.", info.dir.as_deref().unwrap_or_default()),
            "Build it there (`just build`), or unset GITPROMPT_DASHBOARD to use the embedded one.",
        ),
        (DashboardSource::Embedded, false) => {
            found.note("This binary has no dashboard built in: the browser shows a 'not built' page.", "Run `just build`.");
        }
        _ => {}
    }
}

fn specs(report: &Report, found: &mut Found) {
    for problem in &report.specs.problems {
        let fix = match &problem.fallback {
            Some(id) => format!("Fix or delete the file; the bundled {id} spec is used meanwhile."),
            None => "Fix or delete the file; its source is unavailable meanwhile.".to_owned(),
        };
        found.problem(format!("Spec {}: {}", problem.file.path, problem.error), fix);
    }
}

fn connectors(report: &Report, found: &mut Found) {
    for c in &report.connectors {
        let name = &c.name;
        match c.state {
            ConnectorState::Outdated => found.problem(
                format!("{name}: the installed hooks are outdated."),
                "Connectors page > Update (reconnects with the current spec).",
            ),
            ConnectorState::Drift => found.problem(
                format!("{name}: prompts arrived without text (payload drift)."),
                "Update GitPrompt or the spec, then Connectors page > Remap.",
            ),
            ConnectorState::BlockedByPolicy if c.connected => found.problem(
                format!("{name}: hooks are installed, but a setting GitPrompt won't override turns them off."),
                "Change that setting in the tool (or ask your admin), or disconnect.",
            ),
            ConnectorState::Quiet => {
                found.note(format!("{name}: nothing received for over a day."), "Use the tool once; if nothing arrives, reconnect.");
            }
            _ => {}
        }
        if let Some(HookForm::Forwarder { path: Some(path) }) = &c.install
            && !std::path::Path::new(path).is_file()
        {
            found.problem(
                format!("{name}: the hooks run {path}, which does not exist (app moved or removed?)."),
                "Reconnect from the desktop app (Connectors page > Update).",
            );
        }
    }
}

fn spool(report: &Report, found: &mut Found) {
    if report.spool.is_empty() {
        return;
    }
    let total: u64 = report.spool.iter().map(|s| s.payloads).sum();
    let per_source: Vec<String> = report.spool.iter().map(|s| format!("{} {}", s.source, s.payloads)).collect();
    let fix = if report.daemon.running.is_some() {
        "`gitprompt spool drain` delivers them now."
    } else {
        "Start GitPrompt: it delivers them at start (or run `gitprompt spool drain` once it runs)."
    };
    found.problem(format!("{total} spooled hook payloads wait for delivery ({}).", per_source.join(", ")), fix);
}
