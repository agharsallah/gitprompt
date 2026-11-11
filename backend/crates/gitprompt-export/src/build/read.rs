//! Reading values: what a builder writes for a key, a number, an instant, a tool's facts, and the
//! charts, filter descriptions and notes every builder shares.

use indexmap::IndexMap;
use jiff::Timestamp;
use jiff::civil::{Date, DateTime, Time};
use jiff::tz::TimeZone;
use serde::Serialize;

use gitprompt_store::PromptFilter;

use super::{DASH, TOP, json, present};
use crate::input::{Diffstat, Facts, Phase, PhaseReason};
use crate::redact::Options;
use crate::report::{Bar, Chart, ChartKind};
use crate::{Kind, Row, Zone};

// --- labels --------------------------------------------------------------------------------------

const OUTCOMES: &[(&str, &str)] = &[
    ("done", "Done"),
    ("failed", "Failed"),
    ("interrupted", "Interrupted"),
    ("superseded", "Superseded"),
    ("running", "Running"),
    ("open", "No end seen"),
];
const ORIGINS: &[(&str, &str)] = &[
    ("typed", "Typed"),
    ("slash_command", "Slash command"),
    ("task_notification", "Task notification"),
    ("agent_message", "Agent message"),
    ("subagent", "Subagent"),
    ("programmatic", "Programmatic"),
    ("unknown", "Unknown"),
];
const MARKERS: &[(&str, &str)] = &[("subagent_prompt", "Subagent prompt"), ("compaction", "Compaction")];

fn lookup(table: &[(&str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, label)| *label)
}

/// How a turn ended, as read: `done` → `Done`, `open` → `No end seen`; an unknown key as is.
pub fn outcome_label(key: &str) -> String {
    lookup(OUTCOMES, key).unwrap_or(key).to_string()
}

/// Who wrote a prompt, as read: `slash_command` → `Slash command`; an unknown key as is.
pub fn origin_label(key: &str) -> String {
    lookup(ORIGINS, key).unwrap_or(key).to_string()
}

/// A story marker's kind, as read: the origin labels plus `subagent_prompt` → `Subagent prompt` and
/// `compaction` → `Compaction`; an unknown key as is.
pub fn marker_label(key: &str) -> String {
    known_marker(key).unwrap_or(key).to_string()
}

/// A marker kind's label, `None` for an unknown kind.
pub(super) fn known_marker(key: &str) -> Option<&'static str> {
    lookup(ORIGINS, key).or_else(|| lookup(MARKERS, key))
}

/// A unit enum's serde name (`Origin::SlashCommand` → `slash_command`).
fn serde_name(value: impl Serialize) -> String {
    json(value).as_str().unwrap_or_default().to_string()
}

// --- numbers -------------------------------------------------------------------------------------

/// `digits` (ASCII, no sign) with a comma every three digits from the right.
fn group(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A whole number with thousands separators.
fn thousands(n: i64) -> String {
    let sign = if n < 0 { "-" } else { "" };
    format!("{sign}{}", group(&n.unsigned_abs().to_string()))
}

/// Python's `round`: halves to even. The cast saturates, which no count reaches.
#[expect(clippy::cast_possible_truncation, reason = "saturates, which no count reaches")]
fn round(n: f64) -> i64 {
    n.round_ties_even() as i64
}

/// A count, rounded, with thousands separators; `None` is `0`.
pub fn integer(n: Option<f64>) -> String {
    thousands(round(n.unwrap_or(0.0)))
}

/// `$1,234.57`; `<$0.01` for a cost under a cent but not zero; `None` and zero are `$0.00`.
pub fn money(usd: Option<f64>) -> String {
    let usd = usd.unwrap_or(0.0);
    if usd != 0.0 && usd < 0.01 {
        return "<$0.01".to_string();
    }
    let fixed = format!("{usd:.2}");
    let (whole, cents) = fixed.split_once('.').unwrap_or((&fixed, "00"));
    format!("${}.{cents}", group(whole))
}

/// A cost the tool reported, or [`DASH`] when it reported none (not `$0.00`).
pub fn reported(usd: Option<f64>) -> String {
    usd.map_or_else(|| DASH.to_string(), |usd| money(Some(usd)))
}

/// `1 commit`, `2 commits`, `1,200 files`; `many` replaces the `s` plural.
pub fn plural(n: i64, word: &str, many: Option<&str>) -> String {
    let word = if n == 1 { word.to_string() } else { many.map_or_else(|| format!("{word}s"), str::to_string) };
    format!("{} {word}", thousands(n))
}

/// `1 commit, +3 −0 in 1 file`, or `No commits`.
pub fn changes(diffstat: Option<&Diffstat>) -> String {
    match diffstat {
        None => "No commits".to_string(),
        Some(d) => format!(
            "{}, +{} −{} in {}",
            plural(d.commits, "commit", None),
            thousands(d.insertions),
            thousands(d.deletions),
            plural(d.files, "file", None)
        ),
    }
}

/// A duration as read: `42 s`, `3 min` (whole minutes, rounded down), `2 h 5 min`, `12 h` (minutes
/// shown under 10 h only), `1 h` (no `0 min`); [`DASH`] for `None`.
pub fn duration(seconds: Option<f64>) -> String {
    let Some(seconds) = seconds else { return DASH.to_string() };
    let s = round(seconds);
    if s < 60 {
        return format!("{s} s");
    }
    let minutes = s.div_euclid(60);
    if s < 3600 {
        return format!("{minutes} min");
    }
    let (h, m) = (minutes.div_euclid(60), minutes.rem_euclid(60));
    if m != 0 && h < 10 { format!("{h} h {m} min") } else { format!("{h} h") }
}

// --- time ----------------------------------------------------------------------------------------

/// An ISO-8601 value in the reader's zone: an instant (with an offset or `Z`) is converted, a value
/// without one (a naive time, a day) is already local. `None` when it isn't ISO-8601.
fn local(iso: &str, zone: Zone) -> Option<DateTime> {
    if let Ok(at) = iso.parse::<Timestamp>() {
        return Some(match zone {
            Zone::Local => at.to_zoned(TimeZone::system()).datetime(),
            Zone::Fixed(offset) => offset.to_datetime(at),
        });
    }
    iso.parse::<DateTime>().ok().or_else(|| iso.parse::<Date>().ok().map(|day| day.to_datetime(Time::midnight())))
}

/// An instant in the reader's time zone: `2026-03-02 09:00`, or `2026-03-02` when `date_only`. A
/// value without an offset (a day key, a naive time) is already local. [`DASH`] for none or empty;
/// a value that isn't ISO-8601 is shown as is.
pub fn when(iso: Option<&str>, date_only: bool, zone: Zone) -> String {
    let Some(iso) = present(iso) else { return DASH.to_string() };
    match local(iso, zone) {
        Some(at) => at.strftime(if date_only { "%Y-%m-%d" } else { "%Y-%m-%d %H:%M" }).to_string(),
        None => iso.to_string(),
    }
}

/// A time span as read: `2026-03-02 09:00 – 09:03` within a day, both dates otherwise, one time
/// when both read the same or there's no end, `""` without a start.
pub fn span(first: Option<&str>, last: Option<&str>, zone: Zone) -> String {
    let Some(first) = present(first) else { return String::new() };
    let a = when(Some(first), false, zone);
    let Some(last) = present(last) else { return a };
    let b = when(Some(last), false, zone);
    if a == b {
        return a;
    }
    match (a.get(..10), b.get(..10), b.get(11..)) {
        (Some(day_a), Some(day_b), Some(time_b)) if day_a == day_b => format!("{a} – {time_b}"),
        _ => format!("{a} – {b}"),
    }
}

/// Seconds from `start` to `end`, both ISO-8601 (both with an offset, or both naive).
pub(super) fn seconds_between(start: &str, end: &str) -> Option<f64> {
    if let (Ok(a), Ok(b)) = (start.parse::<Timestamp>(), end.parse::<Timestamp>()) {
        return Some(b.duration_since(a).as_secs_f64());
    }
    let (a, b) = (start.parse::<DateTime>().ok()?, end.parse::<DateTime>().ok()?);
    Some(a.duration_until(b).as_secs_f64())
}

// --- facts ---------------------------------------------------------------------------------------

/// The tool that sent a prompt, with the app it was typed in when known:
/// `Claude Code (Desktop app)`; [`DASH`] without a connector label.
pub fn tool(facts: Option<&Facts>) -> String {
    let Some(facts) = facts else { return DASH.to_string() };
    let Some(label) = present(facts.connector.as_ref().and_then(|c| c.label.as_deref())) else { return DASH.to_string() };
    match present(facts.surface.as_ref().and_then(|s| s.name.as_deref())) {
        Some(surface) => format!("{label} ({surface})"),
        None => label.to_string(),
    }
}

/// The turn's model, or [`DASH`].
pub fn model(facts: Option<&Facts>) -> String {
    present(facts.and_then(|f| f.model.as_deref())).unwrap_or(DASH).to_string()
}

/// Tokens the tool reported (their total, as an [`integer`]), or [`DASH`] when it reported none.
pub fn tokens(facts: Option<&Facts>) -> String {
    match facts.and_then(|f| f.tokens.as_ref()) {
        Some(tokens) => thousands(tokens.total),
        None => DASH.to_string(),
    }
}

/// Facts kept as row columns: `tool` (connector id), `surface` (its id, null when unknown), the
/// [`FACT_COLUMNS`](super::FACT_COLUMNS), `tokens` (total), `cost_usd` (null when not reported) and
/// `reasoning_tokens`. Empty without facts.
pub fn fact_columns(facts: Option<&Facts>) -> Row {
    let Some(f) = facts else { return Row::new() };
    let tokens = f.tokens.as_ref();
    [
        ("tool", json(f.connector.as_ref().and_then(|c| c.id.as_ref()))),
        ("surface", json(f.surface.as_ref().and_then(|s| s.id.as_ref()))),
        ("model", json(&f.model)),
        ("permission_mode", json(&f.permission_mode)),
        ("tool_version", json(&f.tool_version)),
        ("api_requests", json(f.api_requests)),
        ("tokens", json(tokens.map(|t| t.total))),
        ("cost_usd", json(f.cost_usd)),
        ("reasoning_tokens", json(tokens.map(|t| t.reasoning))),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect()
}

// --- charts --------------------------------------------------------------------------------------

/// A bar chart of the non-zero counts, largest first (ties keep their order), at most [`TOP`].
pub fn bars(id: &str, title: &str, counts: Vec<(String, f64)>, show: &dyn Fn(f64) -> String, caption: &str) -> Chart {
    let mut kept: Vec<(String, f64)> = counts.into_iter().filter(|(_, value)| *value != 0.0).collect();
    kept.sort_by(|a, b| b.1.total_cmp(&a.1));
    let items = kept.into_iter().take(TOP).map(|(label, value)| Bar { label, value, display: show(value) }).collect();
    Chart { id: id.to_string(), title: title.to_string(), kind: ChartKind::Bars, items, caption: caption.to_string() }
}

/// [`integer`] as a chart's `show`.
pub(super) fn count(value: f64) -> String {
    integer(Some(value))
}

/// Counts as chart values.
pub(super) fn counted<'a>(counts: impl IntoIterator<Item = (String, &'a i64)>) -> Vec<(String, f64)> {
    counts.into_iter().map(|(label, n)| (label, *n as f64)).collect()
}

/// Chart `outcomes`, "How the turns ended", labelled with [`outcome_label`].
pub fn outcome_chart(outcomes: &IndexMap<String, i64>) -> Chart {
    bars("outcomes", "How the turns ended", counted(outcomes.iter().map(|(k, n)| (outcome_label(k), n))), &count, "")
}

// --- filters and notes ---------------------------------------------------------------------------

/// `text` with its first character upper case and the rest lower case (Python's `capitalize`).
fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect(),
        None => String::new(),
    }
}

/// The filters an Overview or a prompt selection was made with, as read, joined by ` · `:
/// `2026-03-01 to now · Typed, slash command · project shop · outcome done, no end seen ·
/// with tool failure · starred · matching “fix”`; `All time` without dates.
pub fn describe_filters(filters: &PromptFilter, zone: Zone) -> String {
    let (since, until) = (present(filters.since.as_deref()), present(filters.until.as_deref()));
    let mut parts = vec![match (since, until) {
        (None, None) => "All time".to_string(),
        _ => format!(
            "{} to {}",
            since.map_or_else(|| "the start".to_string(), |s| when(Some(s), true, zone)),
            until.map_or_else(|| "now".to_string(), |u| when(Some(u), true, zone))
        ),
    }];
    if !filters.origin.is_empty() {
        let origins: Vec<String> = filters.origin.iter().map(|o| origin_label(&serde_name(o))).collect();
        parts.push(capitalize(&origins.join(", ")));
    }
    let models = filters.model.join(", ");
    let named = [
        ("project", filters.repo.as_deref()),
        ("source", filters.source.as_deref()),
        ("model", Some(models.as_str())),
        ("tool", filters.tool.as_deref()),
        ("tag", filters.tag.as_deref()),
    ];
    parts.extend(named.into_iter().filter_map(|(label, value)| present(value).map(|v| format!("{label} {v}"))));
    if !filters.outcome.is_empty() {
        let outcomes: Vec<String> = filters.outcome.iter().map(|o| outcome_label(o.as_str()).to_lowercase()).collect();
        parts.push(format!("outcome {}", outcomes.join(", ")));
    }
    if !filters.has.is_empty() {
        let has: Vec<String> = filters.has.iter().map(|h| serde_name(h).replace('_', " ")).collect();
        parts.push(format!("with {}", has.join(", ")));
    }
    if filters.starred == Some(true) {
        parts.push("starred".to_string());
    }
    if let Some(q) = present(filters.q.as_deref()) {
        parts.push(format!("matching “{q}”"));
    }
    parts.join(" · ")
}

/// What the export includes and leaves out: printed in the file itself, so its reader knows too.
/// The stored-payload note is only part of a `prompts` export.
pub fn notes(options: &Options, kind: Kind) -> Vec<String> {
    let pick = |on: bool, yes: &str, no: &str| if on { yes } else { no }.to_string();
    let mut notes = vec![
        pick(
            options.prompt_text,
            "Includes the text you typed (clipped as in the dashboard), and system text quoted as such.",
            "Figures only: the text of your prompts, system messages, subagent briefs and session titles is left out.",
        ),
        "Secrets that look like keys, tokens or passwords are always replaced by [hidden …].".to_string(),
        pick(
            options.relative_paths,
            "Paths are relative to the project folder, and your home folder is shown as ~.",
            "Paths are absolute, as captured.",
        ),
        pick(
            options.drop_identity,
            "Account email, user and organization ids, commit authors and email addresses are left out.",
            "Includes account identity from telemetry and commit authors.",
        ),
    ];
    if kind == Kind::Prompts {
        notes.push(pick(options.drop_raw, "Stored hook payloads are left out.", "Includes each prompt's stored hook payload."));
    }
    notes.push("Made by GitPrompt on this computer. Nothing was uploaded.".to_string());
    notes
}

/// Why a project phase starts: `After a pause of 3 h`, `On branch feat`, `Branch changed`, `Start`.
pub fn phase_reason(phase: &Phase) -> String {
    match phase.reason {
        PhaseReason::Gap => format!("After a pause of {}", duration(phase.gap_s)),
        PhaseReason::Branch => phase.branches.first().map_or_else(|| "Branch changed".to_string(), |b| format!("On branch {b}")),
        PhaseReason::Start => "Start".to_string(),
    }
}
