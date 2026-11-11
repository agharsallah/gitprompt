//! A session: its summary, telemetry environment and story, one section per chapter and per group
//! of markers.

use indexmap::IndexMap;

use super::read::{count, counted, known_marker, seconds_between};
use super::{DASH, commit_line, fact_columns, figure, json, merge, present, report, section, tally};
use super::{bars, changes, duration, integer, money, outcome_chart, outcome_label, plural, span, tokens, when};
use crate::input::{Chapter, Marker, SessionExport, StoryItem};
use crate::redact::Options;
use crate::report::{Block, Figures, Items, Level, Paragraph, Quote, Report, Section, Voice};
use crate::{Kind, Row, Zone};

/// A session: summary (figures, outcomes, tool kinds), the telemetry environment, then one level-3
/// section per chapter and per group of markers. Rows: one per chapter and per marker.
pub fn session(data: &SessionExport, options: &Options, zone: Zone) -> (Report, Vec<Row>) {
    let story = &data.story;
    let mut sections = vec![summary(data)];
    if !data.environment.is_empty() {
        sections.push(environment(&data.environment));
    }
    let mut rows = Vec::new();
    for item in &story.items {
        match item {
            StoryItem::Chapter(chapter) => {
                sections.push(chapter_section(chapter, zone));
                rows.push(chapter_row(chapter, *options));
            }
            StoryItem::Markers { items } => {
                sections.push(markers_section(items, zone));
                rows.extend(items.iter().map(marker_row));
            }
        }
    }
    let title = present(story.title.as_deref())
        .map_or_else(|| format!("Session {}", story.session.chars().take(8).collect::<String>()), str::to_string);
    let branches = story.branches.join(", ");
    let span = span(story.started_at.as_deref(), story.ended_at.as_deref(), zone);
    let place = [story.project.as_ref().and_then(|p| p.name.as_deref()), Some(branches.as_str()), Some(span.as_str())];
    let subtitle = place.into_iter().filter_map(present).collect::<Vec<_>>().join(" · ");
    (report(Kind::Session, &data.generated_at, title, subtitle, sections, *options, zone), rows)
}

fn chapters(data: &SessionExport) -> impl Iterator<Item = &Chapter> {
    data.story.items.iter().filter_map(|item| match item {
        StoryItem::Chapter(chapter) => Some(chapter),
        StoryItem::Markers { .. } => None,
    })
}

fn summary(data: &SessionExport) -> Section {
    let story = &data.story;
    let totals = &story.totals;
    let seconds = match (present(story.started_at.as_deref()), present(story.ended_at.as_deref())) {
        (Some(start), Some(end)) => seconds_between(start, end),
        _ => None,
    };
    let tool =
        present(story.connector.as_ref().and_then(|c| c.name.as_deref())).or_else(|| present(story.source.as_deref())).unwrap_or(DASH);
    let models = story.models.keys().map(String::as_str).collect::<Vec<_>>().join(", ");
    let mut figures = vec![
        figure("Tool", tool),
        figure("Models", present(Some(models.as_str())).unwrap_or(DASH)),
        figure("Prompts you typed", &integer(Some(totals.chapters as f64))),
        figure("System items", &integer(Some(totals.markers as f64))),
        figure("Span", &duration(seconds)),
    ];
    // As on the page: no usage reported, no cost shown.
    if totals.cost_usd != 0.0 {
        figures.push(figure("Cost", &money(Some(totals.cost_usd))));
    }
    figures.extend([
        figure("Tool calls", &integer(Some(totals.tool_calls as f64))),
        figure("Files written", &integer(Some(totals.files_written as f64))),
        figure("Retries", &integer(Some(totals.retries as f64))),
        figure("Changes", &changes(totals.diffstat.as_ref())),
    ]);
    let mut tools: IndexMap<String, i64> = IndexMap::new();
    for chapter in chapters(data) {
        tally(&mut tools, &chapter.tools);
    }
    let tools_chart = bars("tools", "Tool calls by kind", counted(tools.iter().map(|(k, n)| (k.clone(), n))), &count, "");
    section(
        "Summary",
        vec![Block::Figures(Figures { items: figures }), Block::Chart(outcome_chart(&totals.outcomes)), Block::Chart(tools_chart)],
        String::new(),
        Level::Two,
    )
}

fn environment(environment: &IndexMap<String, String>) -> Section {
    let mut items: Vec<(&String, &String)> = environment.iter().collect();
    items.sort();
    let items = items.into_iter().map(|(k, v)| format!("{k}: {v}")).collect();
    section("Environment", vec![Block::Items(Items { items, mono: true })], "From telemetry".to_string(), Level::Two)
}

fn chapter_section(c: &Chapter, zone: Zone) -> Section {
    let facts = c.facts.as_ref();
    let mut said = vec![outcome_label(&c.outcome)];
    if let Some(seconds) = c.response_s {
        said.push(format!("answered in {}", duration(Some(seconds))));
    }
    if let Some(model) = present(facts.and_then(|f| f.model.as_deref())) {
        said.push(model.to_string());
    }
    if facts.is_some_and(|f| f.tokens.is_some()) {
        said.push(format!("{} tokens", tokens(facts)));
    }
    if let Some(cost) = facts.and_then(|f| f.cost_usd) {
        said.push(money(Some(cost)));
    }
    said.push(plural(c.tool_calls, "tool call", None));
    if c.files_written != 0 {
        said.push(format!("{} written", plural(c.files_written, "file", None)));
    }
    if let Some(retry) = c.retry_of.filter(|id| *id != 0) {
        said.push(format!("retry of #{retry}"));
    }
    let mut blocks = Vec::new();
    if let Some(text) = &c.text {
        let caption = if c.clipped { "clipped" } else { "" };
        blocks.push(Block::Quote(Quote { text: text.clone(), voice: Voice::You, caption: caption.to_string() }));
    }
    if !c.files.is_empty() {
        blocks.push(Block::Items(Items { items: c.files.clone(), mono: true }));
    }
    if !c.commits.is_empty() {
        blocks.push(Block::Items(Items { items: c.commits.iter().map(commit_line).collect(), mono: true }));
    }
    let title = match present(c.slash_command.as_deref()) {
        Some(command) => format!("Prompt {} · {command}", c.turn),
        None => format!("Prompt {}", c.turn),
    };
    let subtitle = format!("{} · {}", when(Some(&c.received_at), false, zone), said.join(" · "));
    section(&title, blocks, subtitle, Level::Three)
}

/// A chapter's row: its figures, commit and diff counts, files, facts (in place) and, with prompt
/// text, its text (`null` when it had none).
fn chapter_row(c: &Chapter, options: Options) -> Row {
    let diffstat = c.diffstat.as_ref();
    let mut row: Row = [
        ("kind", json("chapter")),
        ("id", json(c.id)),
        ("turn", json(c.turn)),
        ("received_at", json(&c.received_at)),
        ("origin", json(&c.origin)),
        ("slash_command", json(&c.slash_command)),
        ("branch", json(&c.branch)),
        ("outcome", json(&c.outcome)),
        ("response_s", json(c.response_s)),
        ("cost_usd", json(c.cost_usd)),
        ("tokens", json(c.tokens)),
        ("tool_calls", json(c.tool_calls)),
        ("tool_failures", json(c.tool_failures)),
        ("files_written", json(c.files_written)),
        ("subagents", json(c.subagents)),
        ("git_commits", json(c.git_commits)),
        ("retry_of", json(c.retry_of)),
        ("commits", json(c.commits.len())),
        ("insertions", json(diffstat.map_or(0, |d| d.insertions))),
        ("deletions", json(diffstat.map_or(0, |d| d.deletions))),
        ("files", json(&c.files)),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect();
    merge(&mut row, fact_columns(c.facts.as_ref()));
    if options.prompt_text {
        row.insert("text".to_string(), json(&c.text));
    }
    row
}

/// A marker's row: what it is, and only the texts it has.
fn marker_row(m: &Marker) -> Row {
    let mut row: Row = [("kind", json(&m.kind)), ("id", json(m.id)), ("received_at", json(&m.received_at))]
        .map(|(k, v)| (k.to_string(), v))
        .into_iter()
        .collect();
    for (key, value) in [("label", &m.label), ("detail", &m.detail), ("excerpt", &m.excerpt)] {
        if let Some(text) = present(value.as_deref()) {
            row.insert(key.to_string(), json(text));
        }
    }
    row
}

/// Consecutive markers: how many of each kind (not typed by you), then each excerpt as system text.
fn markers_section(markers: &[Marker], zone: Zone) -> Section {
    let mut kinds: IndexMap<String, i64> = IndexMap::new();
    for m in markers {
        *kinds.entry(super::marker_label(&m.kind).to_lowercase()).or_default() += 1;
    }
    let counts = kinds.iter().map(|(kind, n)| plural(*n, kind, None)).collect::<Vec<_>>().join(", ");
    let mut blocks = vec![Block::Paragraph(Paragraph { text: format!("{counts}: not typed by you."), muted: true })];
    for m in markers {
        if let Some(excerpt) = present(m.excerpt.as_deref()) {
            let who = [known_marker(&m.kind), present(m.label.as_deref())].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            blocks.push(Block::Quote(Quote { text: excerpt.to_string(), voice: Voice::System, caption: who }));
        }
    }
    let at = markers.first().map(|m| m.received_at.as_str());
    let title = plural(i64::try_from(markers.len()).unwrap_or(i64::MAX), "system item", None);
    section(&title, blocks, when(at, false, zone), Level::Three)
}
