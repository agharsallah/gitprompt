//! The whole session ([`SessionSummary`]): every event that shares the prompt's `session_id`. One small
//! function per group of values, over the session's events.

use std::collections::BTreeSet;

use indexmap::IndexMap;

use gitprompt_core::JsonMap;

use super::{COMMAND_EVENT, ENVIRONMENT, METRIC_PREFIX, SessionSummary, SubagentResult};
use crate::raw::{Event, Raw};
use crate::text::{number, round, text_of};

/// What each value means, by name under `session.` (the Fields page).
pub(super) const DOCS: &[(&str, &str)] = &[
    ("events / first_event_at / last_event_at", "events received for the session, by type, and when"),
    ("starts", "how the session started, then each time it was taken up again (SessionStart.source: startup, resume...)"),
    ("ended / instructions_loaded / cwd_changes / config_changes", "end reason, instruction files, directory and config changes"),
    ("modes / background", "the mode each session start reported (Cursor: composer_mode), and whether it ran as a background agent"),
    ("subagent_results", "each subagent that finished: its type, how it stopped and its last message (SubagentStop)"),
    ("metric_totals", "telemetry metric totals: lines of code, commits, active time, tokens, cost"),
    ("environment", "app version, entrypoint, terminal, OS and account (telemetry resource attributes)"),
];

pub(super) fn summary(events: &[Event<'_>]) -> SessionSummary {
    let mut out = SessionSummary::default();
    activity(events, &mut out);
    lifecycle(events, &mut out);
    subagents(events, &mut out);
    metrics(events, &mut out);
    environment(events, &mut out);
    out
}

fn raws<'e, 'a>(events: &'e [Event<'a>], name: &'e str) -> impl DoubleEndedIterator<Item = &'e Raw<'a>> + 'e {
    events.iter().filter(move |e| e.name() == Some(name)).map(|e| &e.raw)
}

fn text(raw: &Raw<'_>, key: &str) -> Option<String> {
    raw.get(key).and_then(|v| text_of(v.json()))
}

fn truthy_text(raw: &Raw<'_>, keys: &[&str]) -> Option<String> {
    raw.first(keys).and_then(|v| text_of(v.json()))
}

fn activity(events: &[Event<'_>], out: &mut SessionSummary) {
    for e in events {
        *out.events.entry(e.row.event.clone()).or_default() += 1;
    }
    out.first_event_at = events.first().map(|e| e.row.received_at.clone());
    out.last_event_at = events.last().map(|e| e.row.received_at.clone());
}

fn lifecycle(events: &[Event<'_>], out: &mut SessionSummary) {
    let starts: Vec<&Raw<'_>> = raws(events, "SessionStart").collect();
    out.modes = starts.iter().filter_map(|r| truthy_text(r, &["composer_mode"])).collect();
    out.background = starts.iter().any(|r| r.truthy("is_background_agent").is_some());
    out.starts = starts.iter().filter_map(|r| truthy_text(r, &["source"])).collect();
    out.ended = raws(events, "SessionEnd").next_back().and_then(|r| text(r, "reason"));
    let loaded: BTreeSet<String> = raws(events, "InstructionsLoaded").filter_map(|r| text(r, "file_path")).collect();
    out.instructions_loaded = loaded.into_iter().collect();
    out.cwd_changes = raws(events, "CwdChanged").map(|r| text(r, "new_cwd")).collect();
    out.config_changes = raws(events, "ConfigChange").map(|r| truthy_text(r, &["file_path"]).or_else(|| text(r, "source"))).collect();
}

fn subagents(events: &[Event<'_>], out: &mut SessionSummary) {
    out.subagent_results = raws(events, "SubagentStop")
        .map(|r| SubagentResult {
            agent_id: truthy_text(r, &["agent_id"]),
            agent_type: truthy_text(r, &["agent_type", "subagent_type"]),
            stop_reason: truthy_text(r, &["stop_reason", "status"]),
            answer: r.first(&["last_assistant_message", "summary"]).and_then(|v| super::turn::summary_text(v.json())),
        })
        .collect();
}

/// [`super::metric`] of one event.
pub(super) fn metric_of(event: &Event<'_>) -> Option<(String, f64)> {
    let name = event.name().filter(|name| name.starts_with(METRIC_PREFIX))?;
    if !event.raw.has("value") {
        return None;
    }
    let name = match event.raw.truthy("type").and_then(|t| text_of(t.json())) {
        Some(kind) => format!("{name}[{kind}]"),
        None => name.to_owned(),
    };
    Some((name, number(event.raw.get("value").as_ref().map(Raw::json))))
}

/// [`super::command`] of one event.
pub(super) fn command_of(event: &Event<'_>) -> Option<(String, Option<String>)> {
    if event.name() != Some(COMMAND_EVENT) {
        return None;
    }
    let name = truthy_text(&event.raw, &["command_name"])?;
    Some((format!("/{}", name.trim_start_matches('/')), text(&event.raw, "command_source")))
}

fn metrics(events: &[Event<'_>], out: &mut SessionSummary) {
    let mut totals: IndexMap<String, f64> = IndexMap::new();
    for (name, value) in events.iter().filter_map(metric_of) {
        *totals.entry(name).or_default() += value;
    }
    out.metric_totals = totals.into_iter().map(|(name, total)| (name, round(total, 4))).collect();
}

fn environment(events: &[Event<'_>], out: &mut SessionSummary) {
    let Some(latest) = events.iter().rev().find(|e| e.raw.has("session.id")) else { return };
    out.environment = latest
        .raw
        .entries()
        .into_iter()
        .filter(|(key, _)| ENVIRONMENT.iter().any(|prefix| key.starts_with(prefix)))
        .map(|(key, value)| (key.to_owned(), value.json().clone()))
        .collect::<JsonMap>();
}
