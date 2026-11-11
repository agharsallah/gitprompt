//! One session as a narrative ([`super::session_story`]).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use indexmap::IndexMap;

use gitprompt_core::git::Commit;
use gitprompt_core::spec::{ToolKind, by_name};
use gitprompt_store::timeline::TurnEvent;

use super::{
    CHAPTER_CHARS, Chapter, EXCERPT_CHARS, FactRow, MAX_FILES, Marker, MarkerKind, RETRY_WINDOW, SessionItem, SessionStory, SessionTotals,
    TurnTools, attribute_commits, clip, diffstat_of, group_markers, is_user, latest_end, non_empty, project_of, retries,
};
use crate::derivations::files_of;
use crate::text::{round, truthy};

pub(super) fn story(session: &str, rows: &[FactRow], turns: TurnTools<'_>, commits: &[Commit]) -> SessionStory {
    let mut by_turn: HashMap<&str, Vec<&TurnEvent>> = HashMap::new();
    for e in turns.events {
        by_turn.entry(e.prompt_id.as_str()).or_default().push(e);
    }
    let user: Vec<FactRow> = rows.iter().filter(|p| is_user(p)).cloned().collect();
    let owned = attribute_commits(&user, commits);
    let retry = retries(&user, RETRY_WINDOW);
    let mut items: Vec<SessionItem> = Vec::new();
    for (turn, p) in rows.iter().enumerate() {
        let row = &p.prompt.row;
        let events = row.prompt_id.as_deref().and_then(|pid| by_turn.get(pid)).map_or(&[][..], Vec::as_slice);
        if is_user(p) {
            let chapter = Chapter {
                files: files(events, turns),
                commits: owned.get(&row.id).cloned().unwrap_or_default(),
                retry_of: retry.get(&row.id).copied(),
                ..chapter(p, turn + 1, turns.tool_kinds.get(&row.id).cloned().unwrap_or_default())
            };
            items.push(SessionItem::Chapter(Box::new(Chapter { diffstat: diffstat_of(chapter.commits.iter()), ..chapter })));
        } else {
            let kind = crate::origin::parse(&row.origin).map_or(MarkerKind::Unknown, MarkerKind::of_origin);
            items.push(SessionItem::Marker(marker(kind, p, &row.received_at, None, None, row.typed_text.as_deref())));
        }
        items.extend(turn_markers(p, events, turns.kinds).into_iter().map(SessionItem::Marker));
    }
    let chapters: Vec<&Chapter> =
        items.iter().filter_map(|i| if let SessionItem::Chapter(c) = i { Some(c.as_ref()) } else { None }).collect();
    let totals = totals(rows, &items, &chapters, retry.len());
    let first = rows.first();
    let facts = || rows.iter().filter_map(|p| p.facts.as_ref());
    let mut models: IndexMap<String, u64> = IndexMap::new();
    for model in facts().filter_map(|f| non_empty(f.model.as_deref())) {
        *models.entry(model.to_owned()).or_default() += 1;
    }
    let mut branches: Vec<String> = Vec::new();
    for branch in rows.iter().filter_map(|p| non_empty(p.prompt.row.branch.as_deref())) {
        if !branches.iter().any(|b| b == branch) {
            branches.push(branch.to_owned());
        }
    }
    SessionStory {
        session: session.to_owned(),
        title: rows.iter().find_map(|p| non_empty(p.prompt.row.title.as_deref())).map(String::from),
        source: first.map(|p| p.prompt.row.source.clone()),
        connector: first.and_then(|p| p.facts.as_ref()).map(|f| f.connector.clone()),
        surface: facts().find_map(|f| f.surface.clone()),
        models,
        project: project_of(rows),
        branches,
        started_at: first.map(|p| p.prompt.row.received_at.clone()),
        ended_at: latest_end(rows).map(super::super::time::Instant::isoformat),
        totals,
        items: group_markers(items),
    }
}

fn totals(rows: &[FactRow], items: &[SessionItem], chapters: &[&Chapter], retries: usize) -> SessionTotals {
    let mut outcomes = IndexMap::new();
    for c in chapters {
        *outcomes.entry(c.outcome).or_default() += 1;
    }
    let files: BTreeSet<&String> = chapters.iter().flat_map(|c| &c.files).collect();
    // Lengths are far below u64::MAX: the casts are lossless.
    SessionTotals {
        chapters: chapters.len() as u64,
        markers: (items.len() - chapters.len()) as u64,
        cost_usd: round(rows.iter().map(|p| p.prompt.row.usage.cost_usd).sum(), 4),
        tool_calls: rows.iter().map(|p| p.prompt.row.tool_calls).sum(),
        files_written: files.len() as u64,
        outcomes,
        retries: retries as u64,
        diffstat: diffstat_of(chapters.iter().flat_map(|c| &c.commits)),
    }
}

/// A chapter of a prompt you typed, without its files, commits and retry.
fn chapter(p: &FactRow, turn: usize, tools: BTreeMap<ToolKind, i64>) -> Chapter {
    let row = &p.prompt.row;
    let (text, clipped) = clip(row.typed_text.as_deref(), CHAPTER_CHARS);
    Chapter {
        id: row.id,
        turn,
        received_at: row.received_at.clone(),
        origin: row.origin.clone(),
        text,
        clipped,
        slash_command: row.slash_command.clone(),
        branch: row.branch.clone(),
        outcome: p.prompt.outcome,
        response_s: row.response_s,
        cost_usd: row.usage.cost_usd,
        tokens: row.usage.input_tokens + row.usage.output_tokens,
        tool_calls: row.tool_calls,
        tool_failures: row.tool_failures,
        tools,
        files_written: row.files_written,
        files: Vec::new(),
        subagents: row.subagents,
        git_commits: row.git_commits,
        commits: Vec::new(),
        diffstat: None,
        retry_of: None,
        facts: p.facts.clone(),
    }
}

fn marker(kind: MarkerKind, p: &FactRow, received_at: &str, label: Option<&str>, detail: Option<&str>, text: Option<&str>) -> Marker {
    let (excerpt, clipped) = clip(text, EXCERPT_CHARS);
    Marker {
        kind,
        id: p.prompt.row.id,
        received_at: received_at.to_owned(),
        label: label.map(String::from),
        detail: detail.map(String::from),
        excerpt: (!excerpt.is_empty()).then_some(excerpt),
        clipped,
    }
}

fn kind_of(kinds: &IndexMap<String, ToolKind>, tool: Option<&str>) -> ToolKind {
    by_name(kinds, tool).copied().unwrap_or_default()
}

/// The turn's compactions, and the subagents it started with what the model asked them.
fn turn_markers(p: &FactRow, events: &[&TurnEvent], kinds: &IndexMap<String, ToolKind>) -> Vec<Marker> {
    events
        .iter()
        .filter_map(|e| {
            if e.event == "PreCompact" {
                Some(marker(MarkerKind::Compaction, p, &e.received_at, None, None, None))
            } else if kind_of(kinds, e.tool.as_deref()) == ToolKind::Agent && e.prompt.as_deref().is_some_and(|b| !b.is_empty()) {
                let (label, detail) = (e.description.as_deref(), e.subagent_type.as_deref());
                Some(marker(MarkerKind::SubagentPrompt, p, &e.received_at, label, detail, e.prompt.as_deref()))
            } else {
                None
            }
        })
        .collect()
}

/// Sorted paths the turn's write tools touched, at most [`MAX_FILES`].
fn files(events: &[&TurnEvent], turns: TurnTools<'_>) -> Vec<String> {
    let found: BTreeSet<String> = events
        .iter()
        .filter(|e| kind_of(turns.kinds, e.tool.as_deref()) == ToolKind::Write)
        .filter_map(|e| Some((e.tool.as_deref(), e.tool_input.as_ref().filter(|i| truthy(i))?)))
        .flat_map(|(tool, input)| files_of(tool, input, turns.tool_files))
        .collect();
    found.into_iter().take(MAX_FILES).collect()
}
