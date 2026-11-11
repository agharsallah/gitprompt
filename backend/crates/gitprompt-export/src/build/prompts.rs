//! A prompt selection: the prompts the filters keep, newest first.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use indexmap::IndexMap;

use super::read::{count, counted};
use super::{DASH, PROMPT_COLUMNS, RAW_CHARS, fact_columns, fields, figure, json, merge, pick, present, report, section, truthy};
use super::{
    bars, describe_filters, integer, model, money, origin_label, outcome_chart, outcome_label, plural, reported, tokens, tool, when,
};
use crate::input::{PromptItem, PromptsExport};
use crate::redact::Options;
use crate::report::{Block, Figures, Items, Level, Report, Section, Table};
use crate::{Kind, Row, Zone};

/// A prompt selection: summary, a table of prompts, and their stored payloads when collected
/// (each clipped to [`RAW_CHARS`] characters in the document). Rows: one per prompt.
pub fn prompts(data: &PromptsExport, options: &Options, zone: Zone) -> (Report, Vec<Row>) {
    let items = &data.prompts;
    let texts = options.prompt_text && !items.is_empty();
    let mut sections = vec![summary(data), section("Prompts", vec![Block::Table(table(items, texts, zone))], String::new(), Level::Two)];
    let payloads: Vec<String> = items.iter().filter_map(payload_line).collect();
    if !payloads.is_empty() {
        sections.push(section("Stored payloads", vec![Block::Items(Items { items: payloads, mono: true })], String::new(), Level::Two));
    }
    let rows = items.iter().map(|item| row(item, *options)).collect();
    let subtitle = describe_filters(&data.filters, zone);
    (report(Kind::Prompts, &data.generated_at, "Prompts".to_string(), subtitle, sections, *options, zone), rows)
}

fn summary(data: &PromptsExport) -> Section {
    let items = &data.prompts;
    let shown = i64::try_from(items.len()).unwrap_or(i64::MAX);
    let mut prompts = plural(shown, "prompt", None);
    if data.total > shown {
        let _ = write!(prompts, " of {}, the newest", integer(Some(data.total as f64)));
    }
    let sessions: BTreeSet<&str> = items.iter().filter_map(|p| present(p.prompt.row.session.as_deref())).collect();
    let cost: f64 = items.iter().map(|p| p.prompt.row.usage.cost_usd).sum();
    let tool_calls: i64 = items.iter().map(|p| p.prompt.row.tool_calls).sum();
    let mut figures = vec![figure("Prompts", &prompts), figure("Sessions", &integer(Some(sessions.len() as f64)))];
    if cost != 0.0 {
        figures.push(figure("Cost", &money(Some(cost))));
    }
    figures.push(figure("Tool calls", &integer(Some(tool_calls as f64))));

    let mut outcomes: IndexMap<String, i64> = IndexMap::new();
    let mut origins: IndexMap<String, i64> = IndexMap::new();
    for p in items {
        *outcomes.entry(p.prompt.outcome.as_str().to_string()).or_default() += 1;
        *origins.entry(origin_label(&p.prompt.row.origin)).or_default() += 1;
    }
    let origins = bars("origins", "Who wrote the prompts", counted(origins.iter().map(|(k, n)| (k.clone(), n))), &count, "");
    let blocks = vec![Block::Figures(Figures { items: figures }), Block::Chart(outcome_chart(&outcomes)), Block::Chart(origins)];
    section("Summary", blocks, String::new(), Level::Two)
}

fn table(items: &[PromptItem], texts: bool, zone: Zone) -> Table {
    let mut columns: Vec<String> =
        ["When", "Project", "Tool", "Model", "Origin", "Outcome", "Tokens", "Cost", "Tool calls"].map(String::from).to_vec();
    if texts {
        columns.push("Prompt".to_string());
    }
    let rows = items
        .iter()
        .map(|item| {
            let (row, facts) = (&item.prompt.row, item.facts.as_ref());
            let mut cells = vec![
                when(Some(&row.received_at), false, zone),
                present(row.repo_name.as_deref()).or_else(|| present(row.repo.as_deref())).unwrap_or(DASH).to_string(),
                tool(facts),
                model(facts),
                origin_label(&row.origin),
                outcome_label(item.prompt.outcome.as_str()),
                tokens(facts),
                reported(facts.and_then(|f| f.cost_usd)),
                integer(Some(row.tool_calls as f64)),
            ];
            if texts {
                cells.push(row.typed_text.clone().unwrap_or_default());
            }
            cells
        })
        .collect();
    Table { columns, rows, numeric: vec![false, false, false, false, false, false, true, true, true, false] }
}

/// `#<id> <payload as compact JSON>`, clipped to [`RAW_CHARS`] characters; `None` without a payload.
fn payload_line(item: &PromptItem) -> Option<String> {
    let raw = item.raw.as_ref().filter(|raw| truthy(raw))?;
    let clipped: String = raw.to_string().chars().take(RAW_CHARS).collect();
    Some(format!("#{} {clipped}", item.prompt.row.id))
}

/// A prompt's row: the [`PROMPT_COLUMNS`], its facts (in place), then what you typed (with prompt
/// text) and its stored payload (when collected).
fn row(item: &PromptItem, options: Options) -> Row {
    let mut row = pick(&fields(&item.prompt), PROMPT_COLUMNS);
    merge(&mut row, fact_columns(item.facts.as_ref()));
    if options.prompt_text {
        row.insert("typed_text".to_string(), json(&item.prompt.row.typed_text));
    }
    if let Some(raw) = &item.raw {
        row.insert("raw".to_string(), raw.clone());
    }
    row
}
