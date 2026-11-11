//! `gitprompt doctor [--json]`: everything needed to debug an install, read without changing
//! anything (no database, token or folder is created; no tool is run).
//!
//! Sections: version and backend; data directory, database, row counts and index state; the
//! daemon (port, `/healthz`, `/api/status`, lock and its pid, token file); the dashboard
//! `gitprompt serve` would serve; spec files and problems; per connector its state as the
//! Connectors page computes it; the spool backlog; preferences that are set. Then the problems,
//! each with a one-line fix: exit 1 when there is any, else 0.

mod connectors;
mod daemon;
mod findings;
mod render;
mod report;

use jiff::Timestamp;

use gitprompt_core::preferences::{PreferenceFile, ValueSource};
use gitprompt_core::spec::load_spec_set;
use gitprompt_core::{Result, Settings};
use gitprompt_insights::indexer::Indexer;
use gitprompt_server::Dashboard;
use gitprompt_server::status::{self, BACKEND, Counts};
use gitprompt_store::Store;

use crate::Io;
use crate::commands::Failure;
use report::{Data, Finding, PreferenceSet, Report, SpecFileCheck, SpecsCheck};

/// Print the report (text, or JSON with `json`); 0 when healthy, 1 when something needs attention.
pub(crate) fn run(settings: &Settings, json: bool, io: &mut Io<'_>) -> std::result::Result<u8, Failure> {
    let report = gather(settings, Timestamp::now());
    if json {
        serde_json::to_writer_pretty(&mut *io.stdout, &report)?;
        writeln!(io.stdout)?;
    } else {
        render::text(&report, io.stdout)?;
    }
    io.stdout.flush()?;
    Ok(u8::from(!report.healthy))
}

/// Everything the report shows, then the findings derived from it. A part that can't be read
/// (an unreadable database, an invalid preferences file) becomes a problem, not a failure.
fn gather(settings: &Settings, now: Timestamp) -> Report {
    let mut errors = Vec::new();
    let set = load_spec_set(settings);
    let db_path = settings.db_path();
    let store = if db_path.is_file() {
        Store::open(&db_path).map_err(|e| errors.push(Finding::new(format!("The database can't be opened: {e}"), restore_fix()))).ok()
    } else {
        None
    };
    let facts = store.as_ref().map(|store| store_facts(store, &Indexer::new(set.specs.clone())));
    let (stats, counts, db_bytes, index) = match facts {
        Some(Ok(facts)) => facts,
        Some(Err(e)) => {
            errors.push(Finding::new(format!("The database can't be read: {e}"), restore_fix()));
            Default::default()
        }
        None => Default::default(),
    };
    let preferences = PreferenceFile::new(&settings.home, settings.env.clone());
    let preference_list = match preferences.resolve() {
        Ok(resolved) => resolved
            .into_iter()
            .filter(|r| r.source != ValueSource::Default)
            .map(|r| PreferenceSet { key: r.key, value: r.value, source: r.source, env: r.env })
            .collect(),
        Err(e) => {
            let fix = format!("Fix or delete {}.", preferences.path.display());
            errors.push(Finding::new(format!("The preferences can't be read: {e}"), fix));
            Vec::new()
        }
    };
    let spool = status::spool_backlog(settings).unwrap_or_else(|e| {
        errors.push(Finding::new(
            format!("The spool can't be read: {e}"),
            format!("Check the permissions of {}.", settings.spool_dir().display()),
        ));
        Vec::new()
    });
    let token = std::fs::read_to_string(settings.token_file()).ok().map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
    let mut report = Report {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        backend: BACKEND.to_owned(),
        data: Data {
            home: settings.home.display().to_string(),
            home_exists: settings.home.is_dir(),
            db_path: db_path.display().to_string(),
            db_exists: db_path.is_file(),
            db_bytes,
            prompts: counts.prompts,
            events: counts.events,
        },
        index,
        daemon: daemon::check(settings),
        dashboard: Dashboard::from_env(&settings.env).info(),
        specs: SpecsCheck {
            user_dir: settings.user_sources().display().to_string(),
            files: set.files.iter().map(|(id, file)| SpecFileCheck { id: id.clone(), file: file.clone() }).collect(),
            problems: set.problems.clone(),
        },
        connectors: connectors::check(&set, settings, &preferences.effective(), &stats, token.as_deref(), now),
        spool,
        preferences: preference_list,
        problems: Vec::new(),
        notes: Vec::new(),
        healthy: false,
    };
    let (problems, notes) = findings::derive(&report);
    report.problems = errors.into_iter().chain(problems).collect();
    report.notes = notes;
    report.healthy = report.problems.is_empty();
    report
}

type StoreFacts = (std::collections::BTreeMap<String, gitprompt_store::store::SourceStats>, Counts, u64, Option<status::IndexStatus>);

/// Per-source statistics, counts, size and index state of an open store.
fn store_facts(store: &Store, indexer: &Indexer) -> Result<StoreFacts> {
    let stats = store.source_stats()?;
    let counts = status::counts(store)?;
    Ok((stats, counts, store.size()?, Some(status::index_status(store, indexer.version())?)))
}

fn restore_fix() -> String {
    "Quit GitPrompt and restore gitprompt.db from a backup (or move it aside to start fresh).".to_owned()
}
