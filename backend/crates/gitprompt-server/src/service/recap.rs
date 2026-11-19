//! The weekly recap: the week's figures (stats with the key figures: cost, redone work, commits)
//! and its Worth a look, read over the seven days before the instant it is due;
//! [`gitprompt_insights::recap`] says it and says when. Each week handled is remembered in the
//! store, so a restart never sends it twice.

use jiff::{SignedDuration, Timestamp};

use gitprompt_core::spec::{Capability, SpecSet};
use gitprompt_insights::notify::Notification;
use gitprompt_insights::recap::{Week, last_recap, next_recap, recap, week_before};
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::stats::{AggregateQuery, GroupBy};

use super::{Inner, Service};
use crate::error::ApiResult;
use crate::model::Recap;
use crate::query::iso;

/// How long a recap stays the current week's.
const WEEK: SignedDuration = SignedDuration::from_hours(7 * 24);

impl Service {
    /// The recap owed now, sent at most once per week: `None` when it is switched off, this week's
    /// was handled already, or the week held no prompt (that week then counts as handled too).
    pub async fn due_recap(&self) -> ApiResult<Option<Notification>> {
        self.run(Inner::due_recap).await
    }

    /// How long until the next recap is due (the preferences and zone of this moment).
    pub fn until_next_recap(&self) -> SignedDuration {
        let (inner, now) = (&self.inner, self.inner.now());
        next_recap(now, &inner.preferences.effective(), inner.options.zone.as_ref()).duration_since(now)
    }

    /// `GET /api/recap`: the latest recap, while it is the current week's (sent within the
    /// last seven days), if one was sent.
    pub async fn recap(&self) -> ApiResult<Option<Recap>> {
        self.run(|inner| {
            let now = inner.now();
            let Some(row) = inner.store.latest_recap()?.filter(|row| row.due <= now && now.duration_since(row.due) < WEEK) else {
                return Ok(None);
            };
            let Some(text) = row.notification else { return Ok(None) };
            let notification = serde_json::from_str::<Notification>(&text).map_err(gitprompt_core::Error::from)?;
            Ok(Some(Recap { due: iso(row.due), notification }))
        })
        .await
    }
}

impl Inner {
    fn due_recap(&self) -> ApiResult<Option<Notification>> {
        let prefs = self.preferences.effective();
        if !prefs.recap {
            return Ok(None);
        }
        let due = last_recap(self.now(), &prefs, self.options.zone.as_ref());
        if self.store.latest_recap()?.is_some_and(|row| row.due >= due) {
            return Ok(None);
        }
        let notification = self.recap_of(due)?;
        let text = notification.as_ref().map(serde_json::to_string).transpose().map_err(gitprompt_core::Error::from)?;
        self.store.record_recap(due, text.as_deref())?;
        Ok(notification)
    }

    /// The recap of the seven days before `due`.
    fn recap_of(&self, due: Timestamp) -> ApiResult<Option<Notification>> {
        let (set, _) = self.indexed()?;
        let since = week_before(due, self.options.zone.as_ref());
        let filter = PromptFilter { since: Some(iso(since)), until: Some(iso(due)), ..PromptFilter::default() };
        let by_source = self.store.aggregate(&AggregateQuery {
            filter: filter.clone(),
            group_by: Some(GroupBy::Source),
            ..AggregateQuery::default()
        })?;
        let reports_cost = by_source.groups.iter().any(|group| group.api_requests > 0.0 && reports_cost(&set, group.key.as_deref()));
        let redone = self.redone_work(&set, &filter)?;
        let week = Week::new(&by_source.totals, Some(&redone), reports_cost);
        Ok(recap(&week, &self.attention(&set, filter)?))
    }
}

/// Whether the connector `source` reports cost (itself or through a layer).
fn reports_cost(set: &SpecSet, source: Option<&str>) -> bool {
    source.is_some_and(|source| set.tool_has(source, Capability::Cost))
}
