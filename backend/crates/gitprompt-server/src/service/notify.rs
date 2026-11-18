//! What the notifier looks at after each capture: the turns that ended, the possible leaks found,
//! the connectors of each tool. The daemon gathers these [`Event`]s;
//! [`gitprompt_insights::notify::decide`] decides which deserve a notification.

use std::collections::{BTreeMap, BTreeSet};

use jiff::{SignedDuration, Timestamp};

use gitprompt_core::preferences::Preferences;
use gitprompt_core::spec::{Spec, SpecSet};
use gitprompt_insights::notify::{self, ConnectorSeen, Event, LeakFound, Notification, TurnEnd};
use gitprompt_store::filter::{Outcome, PromptFilter};

use super::{Inner, Service};
use crate::error::ApiResult;
use crate::query::iso;

/// Newest prompts read on each check for new turns (a burst bigger than this between two checks,
/// such as a spool drain, is not watched).
const TURNS_READ: usize = 100;
/// How often the connectors are looked at, at most (it reads every source's last arrival).
const CONNECTORS_EVERY: SignedDuration = SignedDuration::from_mins(5);

/// What the notifier remembers between checks.
#[derive(Clone, Debug, Default)]
pub struct Watch {
    /// The newest prompt looked at: newer ones are new turns.
    newest: i64,
    /// Turns that had not ended when last looked at.
    open: BTreeSet<i64>,
    /// Findings of prompts received since `since` already seen (notified or not).
    leaks_seen: BTreeSet<String>,
    /// When watching started: findings of older prompts were there before and are never notified.
    since: String,
    /// When the connectors were last looked at.
    connectors_at: Option<Timestamp>,
    /// The rules' memory: rate limits and quiet spells.
    state: notify::State,
}

impl Service {
    /// Start watching from the newest prompt and now: nothing captured before is notified.
    pub async fn watch(&self) -> ApiResult<Watch> {
        self.run(|inner| {
            inner.indexed()?;
            let newest = inner.store.search(&PromptFilter::default(), 1)?.first().map_or(0, |view| view.row.id);
            let mut watch = Watch { newest, since: iso(inner.now()), ..Watch::default() };
            inner.new_leaks(&mut watch)?; // findings of prompts received in the same instant: seen
            Ok(watch)
        })
        .await
    }

    /// The notifications deserved since the last check (or why they could not be read), and
    /// `watch` moved on. An outer error means the watch was lost (the check panicked).
    pub async fn check_notifications(&self, watch: Watch) -> ApiResult<(Watch, ApiResult<Vec<Notification>>)> {
        self.run(move |inner| {
            let mut watch = watch;
            let found = inner.check(&mut watch);
            Ok((watch, found))
        })
        .await
    }
}

impl Inner {
    fn check(&self, watch: &mut Watch) -> ApiResult<Vec<Notification>> {
        let (set, _) = self.indexed()?;
        let (now, prefs) = (self.now(), self.preferences.effective());
        let mut events = self.ended_turns(&set, watch)?;
        events.extend(self.new_leaks(watch)?);
        if watch.connectors_at.is_none_or(|at| now.duration_since(at) >= CONNECTORS_EVERY) {
            watch.connectors_at = Some(now);
            events.extend(self.connectors(&set, &prefs)?);
        }
        Ok(events.iter().filter_map(|event| notify::decide(event, &mut watch.state, now, &prefs)).collect())
    }

    /// The turns that finished or failed since the last check: the new prompts and the ones still
    /// open last time. A turn that is still running stays watched; any other end is let go.
    fn ended_turns(&self, set: &SpecSet, watch: &mut Watch) -> ApiResult<Vec<Event>> {
        let views = self.store.search(&PromptFilter::default(), TURNS_READ)?;
        let fresh: Vec<i64> = views.iter().map(|view| view.row.id).filter(|id| *id > watch.newest).collect();
        watch.newest = fresh.iter().copied().fold(watch.newest, i64::max);
        watch.open.extend(fresh);
        let ids: Vec<i64> = watch.open.iter().copied().collect();
        let outcomes = self.store.outcomes(&ids)?;
        let mut events = Vec::new();
        for id in ids {
            let failed = match outcomes.get(&id) {
                None | Some(Outcome::Running) => continue, // not indexed yet, or still running
                Some(Outcome::Done) => false,
                Some(Outcome::Failed) => true,
                Some(_) => {
                    watch.open.remove(&id);
                    continue;
                }
            };
            watch.open.remove(&id);
            events.extend(self.turn_end(set, id, failed)?);
        }
        Ok(events)
    }

    /// The end of turn `id` as the rules read it; `None` without a duration.
    fn turn_end(&self, set: &SpecSet, id: i64, failed: bool) -> ApiResult<Option<Event>> {
        let Some(row) = self.store.index().row(id)? else { return Ok(None) };
        let Some(seconds) = row.response_s else { return Ok(None) };
        let tool = set.get(&row.source).map_or_else(|| row.source.clone(), |spec| spec.name.clone());
        Ok(Some(Event::TurnEnded(TurnEnd { prompt: id, failed, seconds, text: row.typed_text, tool })))
    }

    /// The open findings of prompts received since watching started, not seen before, oldest first.
    fn new_leaks(&self, watch: &mut Watch) -> ApiResult<Vec<Event>> {
        let filter = PromptFilter { since: Some(watch.since.clone()), ..PromptFilter::default() };
        let mut open = self.leak_board(&filter)?.open;
        open.reverse();
        let fresh = open.into_iter().filter(|leak| watch.leaks_seen.insert(leak.id.clone()));
        Ok(fresh.map(|leak| Event::LeakFound(LeakFound { kind: leak.kind, masked: leak.masked })).collect())
    }

    /// Every enabled connector that sent something, with the newest arrival from the other
    /// connectors of its tool (a connector `part_of` another shares its tool).
    fn connectors(&self, set: &SpecSet, prefs: &Preferences) -> ApiResult<Vec<Event>> {
        let last: BTreeMap<String, Timestamp> =
            self.store.source_stats()?.into_iter().filter_map(|(id, stats)| Some((id, stats.last_at?.parse().ok()?))).collect();
        let specs: Vec<&Spec> = set.specs.values().map(AsRef::as_ref).filter(|spec| !prefs.disabled_sources.contains(&spec.id)).collect();
        let seen = specs.iter().filter_map(|spec| {
            let last_at = *last.get(&spec.id)?;
            let siblings = specs.iter().filter(|other| other.id != spec.id && tool(other) == tool(spec));
            let tool_last_at = siblings.filter_map(|other| last.get(&other.id)).max().copied()?;
            Some(Event::ConnectorSeen(ConnectorSeen { id: spec.id.clone(), name: spec.name.clone(), last_at, tool_last_at }))
        });
        Ok(seen.collect())
    }
}

/// The tool a connector belongs to: the connector it is part of, else itself.
fn tool(spec: &Spec) -> &str {
    spec.part_of.as_deref().unwrap_or(&spec.id)
}
