//! What `gitprompt why` prints for one commit: the verdict on the first line, so it reads at a
//! glance and in a script (`a1b2c3d ← "add retry to the uploader" · session Rust switch · 12 Nov`),
//! then the commit, the prompt's first lines, what a guess rests on, and where to look further.

use jiff::Timestamp;
use jiff::civil::Date;

use gitprompt_insights::stories::{TraceAlternative, TraceHow, TracePrompt};
use gitprompt_server::model::FoundCommit;

use crate::print::{Style, Tone, first_line};

/// Characters of a prompt's first line in the verdict.
const QUOTE: usize = 60;
/// Lines of the prompt shown under the verdict, and their width.
const PROMPT_LINES: usize = 3;
const LINE: usize = 80;
/// Characters of a session id shown when the session has no name.
const SHORT_ID: usize = 8;
/// Characters of a hash shown (what `git log --oneline` shows).
const SHORT_HASH: usize = 7;

/// What the commit is printed with, besides itself.
pub(super) struct Context<'a> {
    /// The name of the commit's session and of the alternative's, when they have one.
    pub session: Option<String>,
    pub alternative_session: Option<String>,
    /// `http://127.0.0.1:<port>`: the dashboard.
    pub dashboard: &'a str,
    pub now: Timestamp,
    pub style: Style,
}

/// Every line, the verdict first.
pub(super) fn lines(found: &FoundCommit, context: &Context<'_>) -> Vec<String> {
    let trace = &found.trace;
    let mut out = vec![verdict(found, context)];
    let label = |name: &str| context.style.paint(Tone::Dim, &format!("{name:<8} "));
    out.push(format!("{}{}  ({})", label("commit"), trace.commit.subject, found.project.name));
    match &trace.prompt {
        Some(prompt) => out.extend(prompt_lines(prompt).into_iter().enumerate().map(|(i, line)| {
            let head = if i == 0 { label("prompt") } else { " ".repeat(9) };
            format!("{head}{line}")
        })),
        None => out.push(format!("{}no prompt's work was running when it was made", label("by hand"))),
    }
    if let Some(why) = likely_because(trace.how, trace.alternative.as_ref(), context) {
        out.push(format!("{}{why}", label("likely")));
    }
    out.push(format!("{}{}", label("open"), dashboard_link(found, context.dashboard)));
    if let Some(url) = &found.url {
        out.push(format!("{}{url}", label("forge")));
    }
    out
}

/// `a1b2c3d ← "first line" · session name · 12 Nov`; `likely` before a guess's quote, `made by
/// hand` in its place.
pub(super) fn verdict(found: &FoundCommit, context: &Context<'_>) -> String {
    let trace = &found.trace;
    let mut parts = vec![format!("{} ← {}", short(&trace.commit.hash), behind(trace.how, trace.prompt.as_ref()))];
    if let Some(session) = trace.session.as_deref().filter(|_| trace.prompt.is_some()) {
        parts.push(format!("session {}", context.style.paint(Tone::Accent, &session_name(session, context.session.as_deref()))));
    }
    parts.extend(day(&trace.commit.time, context.now));
    parts.join(" · ")
}

/// What made the commit, in words.
fn behind(how: TraceHow, prompt: Option<&TracePrompt>) -> String {
    match (how, prompt) {
        (TraceHow::ByHand, _) | (_, None) => "made by hand".into(),
        (TraceHow::Traced, Some(prompt)) => quote(prompt),
        (TraceHow::Likely, Some(prompt)) => format!("likely {}", quote(prompt)),
    }
}

fn quote(prompt: &TracePrompt) -> String {
    format!("\"{}\"", first_line(&prompt.text, QUOTE))
}

/// A guess says what it rests on: the other prompt that could own the commit, else the timing.
fn likely_because(how: TraceHow, alternative: Option<&TraceAlternative>, context: &Context<'_>) -> Option<String> {
    if how != TraceHow::Likely {
        return None;
    }
    let Some(alternative) = alternative else {
        return Some("not certain: committed after the prompt's work ended, or rewritten since (amend, rebase)".into());
    };
    let session = alternative
        .session
        .as_deref()
        .map(|id| format!(" · session {}", session_name(id, context.alternative_session.as_deref())))
        .unwrap_or_default();
    Some(format!("not certain: another prompt was at work then and could have made it: {}{session}", quote(&alternative.prompt)))
}

/// The prompt's first lines with words, each cut to [`LINE`]; `…` when there is more.
fn prompt_lines(prompt: &TracePrompt) -> Vec<String> {
    let all: Vec<&str> = prompt.text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let mut shown: Vec<String> = all.iter().take(PROMPT_LINES).map(|line| first_line(line, LINE)).collect();
    if all.len() > PROMPT_LINES || prompt.clipped {
        shown.push("…".into());
    }
    shown
}

/// The commit's row in its project's Commits tab.
pub(super) fn dashboard_link(found: &FoundCommit, dashboard: &str) -> String {
    format!("{dashboard}/projects/{}?tab=commits#commit-{}", found.project.id, found.trace.commit.hash)
}

/// The name you gave the session (or its label), else the start of its id.
fn session_name(id: &str, name: Option<&str>) -> String {
    name.map_or_else(|| id.chars().take(SHORT_ID).collect(), str::to_owned)
}

pub(super) fn short(hash: &str) -> &str {
    hash.get(..SHORT_HASH).unwrap_or(hash)
}

/// `12 Nov` on the commit's own calendar, with the year when it is not this year; nothing for a
/// time that can't be read.
pub(super) fn day(iso: &str, now: Timestamp) -> Option<String> {
    let date: Date = iso.get(..10)?.parse().ok()?;
    let this_year = now.to_zoned(jiff::tz::TimeZone::UTC).year();
    Some(if date.year() == this_year { date.strftime("%-d %b").to_string() } else { date.strftime("%-d %b %Y").to_string() })
}
