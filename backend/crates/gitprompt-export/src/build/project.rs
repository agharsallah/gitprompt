//! A project: how it got built, phase by phase.

use super::read::{count, counted};
use super::{DASH, commit_line, fact_columns, fields, figure, json, merge, present, report, section, tools_line};
use super::{bars, changes, integer, model, money, outcome_chart, outcome_label, phase_reason, plural, reported, span, tool, when};
use crate::input::{Measures, Phase, ProjectExport, ProjectPrompt};
use crate::redact::Options;
use crate::report::{Block, Figures, Items, Level, Paragraph, Report, Section, Table};
use crate::{Kind, Row, Zone};

/// Prompt keys a project row leaves out or writes its own way.
const NOT_COPIED: &[&str] = &["commits", "tools", "clipped", "facts", "text"];
/// A phase table's columns before `Prompt` (shown with prompt text).
const COLUMNS: &[&str] = &["When", "Branch", "Tool", "Model", "Outcome", "Cost", "Tools", "Commits"];
/// Right-aligned columns, `Prompt` included.
const NUMERIC: &[bool] = &[false, false, false, false, false, true, false, true, false];

/// A project: summary (measures, prompts per phase, outcomes, a note on other origins), then one
/// section per phase with its table of prompts. Rows: one per prompt, with its phase.
pub fn project(data: &ProjectExport, options: &Options, zone: Zone) -> (Report, Vec<Row>) {
    let story = &data.story;
    let totals = &story.totals;
    let phases = counted(story.phases.iter().map(|p| (format!("Phase {}", p.index), &p.measures.prompts)));
    let other = format!("{} of other origins in this project are counted, not shown.", plural(story.system, "prompt", None));
    let summary = vec![
        measures(totals),
        Block::Chart(bars("phases", "Prompts per phase", phases, &count, "")),
        Block::Chart(outcome_chart(&totals.outcomes)),
        Block::Paragraph(Paragraph { text: other, muted: true }),
    ];
    let mut sections = vec![section("Summary", summary, String::new(), Level::Two)];
    let texts = options.prompt_text && story.phases.iter().any(|p| !p.items.is_empty());
    let mut rows = Vec::new();
    for phase in &story.phases {
        sections.push(phase_section(phase, texts, zone));
        rows.extend(phase.items.iter().map(|prompt| prompt_row(phase.index, prompt, options)));
    }
    let span = span(totals.first_at.as_deref(), totals.last_at.as_deref(), zone);
    let subtitle = [story.repo.as_deref(), Some(span.as_str())].into_iter().filter_map(present).collect::<Vec<_>>().join(" · ");
    let title = present(story.name.as_deref()).unwrap_or("Project").to_string();
    (report(Kind::Project, &data.generated_at, title, subtitle, sections, options, zone), rows)
}

fn measures(m: &Measures) -> Block {
    let n = |v: i64| integer(Some(v as f64));
    Block::Figures(Figures {
        items: vec![
            figure("Prompts", &n(m.prompts)),
            figure("Sessions", &n(m.sessions)),
            figure("Cost", &money(Some(m.cost_usd))),
            figure("Tool calls", &n(m.tool_calls)),
            figure("Files written", &n(m.files_written)),
            figure("Retries", &n(m.retries)),
            figure("Median length", &format!("{} characters", n(m.median_chars))),
            figure("Changes", &changes(m.diffstat.as_ref())),
        ],
    })
}

/// A phase: its measures, its prompts as a table (with what you typed when `texts`), its commits.
fn phase_section(phase: &Phase, texts: bool, zone: Zone) -> Section {
    let mut columns: Vec<String> = COLUMNS.iter().map(|c| c.to_string()).collect();
    if texts {
        columns.push("Prompt".to_string());
    }
    let rows = phase.items.iter().map(|prompt| table_row(prompt, texts, zone)).collect();
    let mut blocks = vec![measures(&phase.measures), Block::Table(Table { columns, rows, numeric: NUMERIC.to_vec() })];
    let commits: Vec<String> = phase.items.iter().flat_map(|p| &p.commits).map(commit_line).collect();
    if !commits.is_empty() {
        blocks.push(Block::Items(Items { items: commits, mono: true }));
    }
    let measures = &phase.measures;
    let subtitle = format!("{} · {}", phase_reason(phase), span(measures.first_at.as_deref(), measures.last_at.as_deref(), zone));
    section(&format!("Phase {}", phase.index), blocks, subtitle, Level::Two)
}

fn table_row(prompt: &ProjectPrompt, texts: bool, zone: Zone) -> Vec<String> {
    let facts = prompt.facts.as_ref();
    let mut row = vec![
        when(Some(&prompt.received_at), false, zone),
        present(prompt.branch.as_deref()).unwrap_or(DASH).to_string(),
        tool(facts),
        model(facts),
        outcome_label(&prompt.outcome),
        reported(facts.and_then(|f| f.cost_usd)),
        tools_line(&prompt.tools),
        prompt.commits.len().to_string(),
    ];
    if texts {
        row.push(prompt.text.clone().unwrap_or_default());
    }
    row
}

/// A prompt's row: its phase, its own fields (with its text only with prompt text), its tool calls
/// per kind, its commit count and its facts (in place).
fn prompt_row(phase: i64, prompt: &ProjectPrompt, options: &Options) -> Row {
    let mut row = Row::new();
    row.insert("phase".to_string(), json(phase));
    for (key, value) in fields(prompt) {
        if !NOT_COPIED.contains(&key.as_str()) || (key == "text" && options.prompt_text) {
            row.insert(key, value);
        }
    }
    row.insert("tools".to_string(), json(&prompt.tools));
    row.insert("commits".to_string(), json(prompt.commits.len()));
    merge(&mut row, fact_columns(prompt.facts.as_ref()));
    row
}
