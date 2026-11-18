//! The prompts behind a branch, for a pull request: a summary line, then each prompt you typed on
//! the branch with the commits made in its work, one section each (folded by
//! [`crate::markdown::paste`], so a long branch does not flood the page).

use std::collections::BTreeSet;

use super::{DASH, commit_line, duration, json, plural, present, section, when};
use crate::input::{BranchExport, ProjectPrompt};
use crate::redact::Options;
use crate::report::{Block, Items, Level, Paragraph, Quote, Report, Section, Voice};
use crate::{Kind, Row, Zone};

/// What a branch block includes: the redaction defaults (secrets hidden, paths relative, no
/// identity, no raw payloads) with the prompt text on, since the block is the prompts.
pub const BRANCH_OPTIONS: Options = Options { prompt_text: true, relative_paths: true, drop_identity: true, drop_raw: true };
/// Characters of a prompt's first line in its heading.
const HEADLINE_CHARS: usize = 72;
/// What the summary line says when no prompt you typed was sent on the branch.
pub const EMPTY: &str = "No prompts you typed were recorded on this branch.";
/// Hashes named in a prompt's heading before `+N`.
const HASHES_SHOWN: usize = 1;

/// The branch's prompts in order, each a level-3 section titled by its number and first line, its
/// commits' short hashes beside; the summary line as subtitle; a one-line footer as notes. Rows:
/// one per prompt.
pub fn branch(data: &BranchExport, options: &Options, zone: Zone) -> (Report, Vec<Row>) {
    let sections = data.prompts.iter().enumerate().map(|(i, prompt)| prompt_section(i + 1, prompt)).collect();
    let report = Report {
        kind: Kind::Branch,
        title: format!("Prompts behind {}", data.branch),
        subtitle: summary(&data.prompts),
        generated_at: when(Some(&data.generated_at), false, zone),
        sections,
        notes: footer(*options),
    };
    (report, data.prompts.iter().enumerate().map(|(i, prompt)| row(i + 1, prompt)).collect())
}

/// `7 prompts · 4 commits · 2 h of agent time · 1 reworded`: agent time only when a tool reported
/// how long a turn ran, rewordings only when there are some (never a figure the data can't back).
fn summary(prompts: &[ProjectPrompt]) -> String {
    if prompts.is_empty() {
        return EMPTY.to_string();
    }
    let commits: BTreeSet<&str> = prompts.iter().flat_map(|p| &p.commits).map(|c| c.hash.as_str()).collect();
    let mut parts = vec![plural(count(prompts.len()), "prompt", None), plural(count(commits.len()), "commit", None)];
    if prompts.iter().any(|p| p.response_s.is_some()) {
        let worked: f64 = prompts.iter().filter_map(|p| p.response_s).sum();
        parts.push(format!("{} of agent time", duration(Some(worked))));
    }
    let reworded = prompts.iter().filter(|p| rewords(p)).count();
    if reworded > 0 {
        parts.push(format!("{reworded} reworded"));
    }
    parts.join(" · ")
}

/// `Secrets masked · paths relative · made by GitPrompt, on this machine`, as the options made it.
fn footer(options: Options) -> Vec<String> {
    let paths = if options.relative_paths { "paths relative" } else { "paths absolute" };
    ["Secrets masked", paths, "made by GitPrompt, on this machine"].map(str::to_string).to_vec()
}

fn prompt_section(n: usize, prompt: &ProjectPrompt) -> Section {
    let title = match present(prompt.text.as_deref()) {
        Some(text) => format!("{n}. {}", headline(text)),
        None => format!("Prompt {n}"),
    };
    let mut blocks = Vec::new();
    if let Some(text) = facts(prompt) {
        blocks.push(Block::Paragraph(Paragraph { text, muted: true }));
    }
    if let Some(text) = present(prompt.text.as_deref()) {
        let caption = if prompt.clipped { "clipped" } else { "" };
        blocks.push(Block::Quote(Quote { text: text.to_string(), voice: Voice::You, caption: caption.to_string() }));
    }
    if !prompt.commits.is_empty() {
        blocks.push(Block::Items(Items { items: prompt.commits.iter().map(commit_line).collect(), mono: true }));
    }
    section(&title, blocks, hashes(prompt), Level::Three)
}

/// How long its turn ran and whether it rewords an earlier prompt; `None` when neither is known.
/// Not when it was sent: the order says enough in a pull request, and a time would be in this
/// machine's zone.
fn facts(prompt: &ProjectPrompt) -> Option<String> {
    let mut said = Vec::new();
    if let Some(seconds) = prompt.response_s {
        said.push(format!("worked {}", duration(Some(seconds))));
    }
    if rewords(prompt) {
        said.push("rewords an earlier prompt".to_string());
    }
    (!said.is_empty()).then(|| said.join(" · "))
}

/// `d3b1663`, `a08db20 +1`; empty without commits.
fn hashes(prompt: &ProjectPrompt) -> String {
    let shown: Vec<String> = prompt.commits.iter().take(HASHES_SHOWN).map(|c| short(&c.hash)).collect();
    let more = prompt.commits.len().saturating_sub(HASHES_SHOWN);
    let more = if more > 0 { format!(" +{more}") } else { String::new() };
    format!("{}{more}", shown.join(" "))
}

/// The first line of the text, at most [`HEADLINE_CHARS`] characters (`…` when cut).
fn headline(text: &str) -> String {
    let line = text.trim().lines().next().unwrap_or_default().trim();
    if line.chars().count() <= HEADLINE_CHARS {
        return if line.is_empty() { DASH.to_string() } else { line.to_string() };
    }
    let cut: String = line.chars().take(HEADLINE_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

fn short(hash: &str) -> String {
    hash.chars().take(7).collect()
}

fn rewords(prompt: &ProjectPrompt) -> bool {
    prompt.retry_of.is_some_and(|id| id != 0)
}

/// A prompt's row: its place, when, how it went, how long, and its commits' short hashes.
fn row(n: usize, p: &ProjectPrompt) -> Row {
    [
        ("n", json(n)),
        ("id", json(p.id)),
        ("received_at", json(&p.received_at)),
        ("outcome", json(&p.outcome)),
        ("response_s", json(p.response_s)),
        ("retry_of", json(p.retry_of)),
        ("commits", json(p.commits.iter().map(|c| short(&c.hash)).collect::<Vec<_>>())),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect()
}

/// Counts are far below `i64::MAX`.
fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}
