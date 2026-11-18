//! Redone work: what the turns you had to do again cost, in money and agent time (decision D3).
//!
//! A turn is redone when it failed, when you interrupted it, or when the next prompt you wrote in
//! its session is a version of it ([`crate::lineage::Shingles::is_version`]): you asked again.
//! A turn counts once, under the first of those reasons that holds, so a failed turn you then
//! reworded is one failed turn. Cost is summed only for tools that report it; a range where no tool
//! does has no cost at all, never a misleading $0.
//!
//! Only turns you started count (typed or a slash command): a turn a task notification or an agent
//! message started is not work you asked for, and its length is the wait for its end. A failed turn
//! that committed is not counted either: its work landed (both found on real data at gate G3).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_store::filter::Outcome;
use gitprompt_store::timeline::IndexedPrompt;

use crate::lineage::Shingles;
use crate::origin;
use crate::stories::USER_ORIGINS;
use crate::text::words;

/// Words of the costliest turn's text kept for its sentence.
const EXCERPT_WORDS: usize = 12;

/// Why a turn was done again, in the order a turn is counted under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "RedoneReason")]
pub enum Reason {
    Failed,
    Interrupted,
    Reworded,
}

/// The redone turns of one reason, or of all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct RedonePart {
    pub turns: i64,
    pub agent_time_s: f64,
    /// Absent when no tool in the range reports cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cost_usd: Option<f64>,
}

/// The redone turn that cost the most (money where reported, else agent time).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CostliestTurn {
    /// The prompt id of the turn.
    pub id: i64,
    pub reason: Reason,
    pub agent_time_s: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cost_usd: Option<f64>,
    /// The first words you typed, when it was typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
}

/// What the turns you redid cost, split by why.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct RedoneWork {
    #[serde(flatten)]
    pub total: RedonePart,
    pub failed: RedonePart,
    pub interrupted: RedonePart,
    pub reworded: RedonePart,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub costliest: Option<CostliestTurn>,
}

/// One turn as the rule sees it.
struct Turn<'a> {
    prompt: &'a IndexedPrompt,
    /// Its cost, when its tool reports cost.
    cost: Option<f64>,
}

impl Turn<'_> {
    fn agent_time(&self) -> f64 {
        self.prompt.row.response_s.unwrap_or(0.0)
    }

    /// You started it: typed, or a slash command.
    fn by_you(&self) -> bool {
        origin::parse(&self.prompt.row.origin).is_some_and(|o| USER_ORIGINS.contains(&o))
    }

    fn typed(&self) -> Option<&str> {
        self.prompt.row.typed_text.as_deref().filter(|text| self.by_you() && !text.trim().is_empty())
    }
}

/// The redone work among `prompts` (any order); `reports_cost(source)` says whether a connector
/// reports cost, and a turn's cost counts only when its usage arrived.
pub fn redone(prompts: &[IndexedPrompt], reports_cost: impl Fn(&str) -> bool) -> RedoneWork {
    let mut turns: Vec<Turn<'_>> = prompts
        .iter()
        .map(|prompt| {
            let usage = &prompt.row.usage;
            let cost = (reports_cost(&prompt.row.source) && usage.api_requests > 0).then_some(usage.cost_usd);
            Turn { prompt, cost }
        })
        .collect();
    turns.sort_by(|a, b| (&a.prompt.row.session, a.prompt.row.id).cmp(&(&b.prompt.row.session, b.prompt.row.id)));
    let costed = turns.iter().any(|t| t.cost.is_some());
    let reasons = reasons(&turns);

    let mut work = RedoneWork::default();
    let mut costliest: Option<(&Turn<'_>, Reason)> = None;
    for (turn, reason) in turns.iter().zip(reasons) {
        let Some(reason) = reason else { continue };
        let part = match reason {
            Reason::Failed => &mut work.failed,
            Reason::Interrupted => &mut work.interrupted,
            Reason::Reworded => &mut work.reworded,
        };
        add(part, turn, costed);
        add(&mut work.total, turn, costed);
        if costliest.is_none_or(|(best, _)| weight(turn) > weight(best)) {
            costliest = Some((turn, reason));
        }
    }
    work.costliest = costliest.map(|(turn, reason)| CostliestTurn {
        id: turn.prompt.row.id,
        reason,
        agent_time_s: turn.agent_time(),
        cost_usd: turn.cost,
        text: turn.typed().map(|text| words(text).take(EXCERPT_WORDS).collect::<Vec<_>>().join(" ")),
    });
    work
}

/// Why each turn (sorted by session, then id) was redone, if it was.
fn reasons(turns: &[Turn<'_>]) -> Vec<Option<Reason>> {
    let reworded = reworded(turns);
    turns
        .iter()
        .zip(reworded)
        .map(|(turn, reworded)| match turn.prompt.outcome {
            _ if !turn.by_you() => None,
            Outcome::Failed if turn.prompt.row.git_commits > 0 => reworded.then_some(Reason::Reworded),
            Outcome::Failed => Some(Reason::Failed),
            Outcome::Interrupted => Some(Reason::Interrupted),
            _ => reworded.then_some(Reason::Reworded),
        })
        .collect()
}

/// For each turn, whether the next typed prompt of its session is a version of it.
fn reworded(turns: &[Turn<'_>]) -> Vec<bool> {
    let mut out = vec![false; turns.len()];
    let mut previous: Option<(usize, Shingles)> = None;
    for (at, turn) in turns.iter().enumerate() {
        let Some(text) = turn.typed() else { continue };
        let shingles = Shingles::new(text);
        if let Some((before, ref earlier)) = previous
            && turns.get(before).is_some_and(|b| b.prompt.row.session == turn.prompt.row.session)
            && earlier.is_version(&shingles)
            && let Some(slot) = out.get_mut(before)
        {
            *slot = true;
        }
        previous = Some((at, shingles));
    }
    out
}

fn add(part: &mut RedonePart, turn: &Turn<'_>, costed: bool) {
    part.turns += 1;
    part.agent_time_s = gitprompt_core::numbers::round(part.agent_time_s + turn.agent_time(), 1);
    if costed {
        part.cost_usd = Some(gitprompt_core::numbers::round(part.cost_usd.unwrap_or(0.0) + turn.cost.unwrap_or(0.0), 4));
    }
}

/// How much a turn weighs for "costliest": its cost first, then its agent time.
fn weight(turn: &Turn<'_>) -> (f64, f64) {
    (turn.cost.unwrap_or(0.0), turn.agent_time())
}
