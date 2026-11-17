//! Find: the best matches of each kind (prompts, sessions, projects) for one query, ranked the same
//! way for every kind. Pure: callers pass what the store read ([`IndexedPrompt`] rows, the session
//! and project groups of a statistics read); nothing here reads a database.
//!
//! Ranking, best first (an empty query skips the first two: the most recent of each kind):
//!
//! 1. text match: the text is the query itself (3), contains it as a phrase (2), or contains every
//!    word of it somewhere (1)
//! 2. outcome (prompts only): finished first, see `outcome_rank`
//! 3. recency: the newest first; equal times keep the order the caller read them in
//!
//! At most [`PER_KIND`] hits per kind. A kind is one small function returning `Vec<Hit>` over its
//! own candidates ([`prompt_hits`], [`session_hits`], [`project_hits`]); a new kind (commits,
//! chapters) adds a function and a [`Kind`] and never touches `rank`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_store::filter::Outcome;
use gitprompt_store::stats::Bucket;
use gitprompt_store::timeline::IndexedPrompt;

use crate::snippets::{Segment, WIDTH, highlight};
use crate::stories::{folder_name, project_id};
use crate::text::words;

/// Hits per kind, at most.
pub const PER_KIND: usize = 5;

/// A kind of thing find looks for, by the name of its list in [`FindResults`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Prompts,
    Sessions,
    Projects,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Prompts, Kind::Sessions, Kind::Projects];
}

/// What a hit opens: the id the dashboard builds the link from, as its story route takes it (a
/// prompt's number, a session's id, a project's id).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[ts(rename = "FindTarget")]
pub enum Target {
    Prompt { id: i64 },
    Session { id: String },
    Project { id: String },
}

/// One match.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(rename = "FindHit")]
pub struct Hit {
    pub target: Target,
    /// What to call it: the session's title (a prompt's session, a session) or the project's name.
    pub title: Option<String>,
    /// The matched text around the query's words, matched words marked.
    pub snippet: Vec<Segment>,
    /// When it was last active (a prompt: when it was sent).
    pub at: String,
    /// Prompts only: how the turn ended.
    pub outcome: Option<Outcome>,
}

/// The best matches of each kind; a kind that was not asked for is empty.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[ts(rename = "FindResults")]
pub struct FindResults {
    pub prompts: Vec<Hit>,
    pub sessions: Vec<Hit>,
    pub projects: Vec<Hit>,
}

/// A thing that matched, with what ranking needs.
struct Candidate {
    target: Target,
    title: Option<String>,
    /// The text the query matched, the snippet's source.
    text: String,
    at: String,
    outcome: Option<Outcome>,
    score: u8,
    /// 0 is best; always 0 without a query.
    outcome_rank: u8,
}

// --- the kinds ---------------------------------------------------------------------------------

/// Prompts the store already matched on their typed text (`q` as the prompt filter), best first,
/// at most [`PER_KIND`].
pub fn prompt_hits(rows: &[IndexedPrompt], q: &str, limit: usize) -> Vec<Hit> {
    prompt_hits_up_to(rows, q, limit.min(PER_KIND))
}

/// [`prompt_hits`] with the caller's own bound: a list that is read in full (the terminal's
/// `gitprompt find`) wants more than the palette's few.
pub fn prompt_hits_up_to(rows: &[IndexedPrompt], q: &str, limit: usize) -> Vec<Hit> {
    let ranked_by_outcome = !q.trim().is_empty();
    let candidates = rows.iter().map(|r| {
        let text = r.row.typed_text.clone().unwrap_or_default();
        // The store's filter already decided it matches; a word it matched but this does not (case
        // folding differences) still counts as the weakest match.
        let score = text_score(&text, q).unwrap_or(1);
        Candidate {
            target: Target::Prompt { id: r.row.id },
            title: r.row.title.clone().filter(|t| !t.is_empty()),
            text,
            at: r.row.received_at.clone(),
            outcome: Some(r.outcome),
            score,
            outcome_rank: if ranked_by_outcome { outcome_rank(r.outcome) } else { 0 },
        }
    });
    rank(candidates.collect(), q, limit)
}

/// Sessions (the groups of a statistics read grouped by session, named) matched on the best of
/// [`session_texts`], and called by [`session_title`].
pub fn session_hits(groups: &[Bucket], q: &str, limit: usize) -> Vec<Hit> {
    let limit = limit.min(PER_KIND);
    let candidates = groups.iter().filter_map(|g| {
        let id = g.key.clone().filter(|k| !k.is_empty())?;
        let (score, text) = session_texts(g).filter_map(|text| Some((text_score(text, q)?, text))).max_by_key(|(score, _)| *score)?;
        let title = session_title(g);
        Some(Candidate {
            target: Target::Session { id },
            title,
            text: text.to_owned(),
            at: g.last_at.clone().unwrap_or_default(),
            outcome: None,
            score,
            outcome_rank: 0,
        })
    });
    rank(candidates.collect(), q, limit)
}

/// Projects (the groups of a statistics read grouped by repo) matched on their name.
pub fn project_hits(groups: &[Bucket], q: &str, limit: usize) -> Vec<Hit> {
    let limit = limit.min(PER_KIND);
    let candidates = groups.iter().filter_map(|g| {
        let repo = g.key.clone().filter(|k| !k.is_empty())?;
        let name = g.detail.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| folder_name(&repo));
        named(Target::Project { id: project_id(&repo) }, name, g.last_at.clone(), q)
    });
    rank(candidates.collect(), q, limit)
}

/// The texts a session is matched on, in the order it is called by: the name you gave it, its
/// default label, then the group's own detail (the title the tool reported, or its first prompt).
/// The one place that says it.
pub fn session_texts(session: &Bucket) -> impl Iterator<Item = &str> {
    [&session.name, &session.label, &session.detail].into_iter().flatten().map(String::as_str).filter(|text| !text.is_empty())
}

/// What a session is called: your name, else its label, else what the tool reported.
pub fn session_title(session: &Bucket) -> Option<String> {
    session_texts(session).next().map(str::to_owned)
}

/// Whether `text` holds every word of `q` (as ranking decides it); an empty query matches nothing
/// here, since there is nothing to look for.
pub fn matches(text: &str, q: &str) -> bool {
    text_score(text, q).is_some_and(|score| score > 0)
}

/// A candidate matched on its own name, or `None` when the query does not match it.
fn named(target: Target, text: String, at: Option<String>, q: &str) -> Option<Candidate> {
    let score = text_score(&text, q)?;
    let title = Some(text.clone()).filter(|t| !t.is_empty());
    Some(Candidate { target, title, text, at: at.unwrap_or_default(), outcome: None, score, outcome_rank: 0 })
}

// --- ranking -----------------------------------------------------------------------------------

/// The best `limit` candidates as hits: best text match, then best outcome, then newest; the sort
/// is stable, so equal ones keep the order they came in.
fn rank(mut candidates: Vec<Candidate>, q: &str, limit: usize) -> Vec<Hit> {
    candidates.sort_by(|a, b| b.score.cmp(&a.score).then(a.outcome_rank.cmp(&b.outcome_rank)).then_with(|| b.at.cmp(&a.at)));
    candidates.truncate(limit);
    candidates
        .into_iter()
        .map(|c| Hit { snippet: highlight(Some(&c.text), Some(q), WIDTH), target: c.target, title: c.title, at: c.at, outcome: c.outcome })
        .collect()
}

/// How well `text` matches `q`: 3 when it is the query, 2 when it contains the query's words in
/// that order and spacing, 1 when it contains every word somewhere (ignoring case); `None` when a
/// word is missing. Any text matches an empty query, with 0.
fn text_score(text: &str, q: &str) -> Option<u8> {
    let query: Vec<String> = words(q).map(str::to_lowercase).collect();
    if query.is_empty() {
        return Some(0);
    }
    let flat = words(text).map(str::to_lowercase).collect::<Vec<_>>().join(" ");
    let phrase = query.join(" ");
    if !query.iter().all(|w| flat.contains(w.as_str())) {
        return None;
    }
    if flat == phrase {
        Some(3)
    } else if flat.contains(&phrase) {
        Some(2)
    } else {
        Some(1)
    }
}

/// How early a turn's outcome sorts, 0 first: finished, then not settled (running, open, moved on
/// from), then stopped by you, then failed.
fn outcome_rank(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Done => 0,
        Outcome::Running | Outcome::Open | Outcome::Superseded => 1,
        Outcome::Interrupted => 2,
        Outcome::Failed => 3,
    }
}
