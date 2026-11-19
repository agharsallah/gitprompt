//! What the desktop tray shows at a glance: today's prompts and cost, the sessions to pick up,
//! the last prompts you typed and how many possible leaks are open. One read for the whole menu;
//! the menu itself is built in `gitprompt-tauri` (`tray_menu`).

use gitprompt_core::spec::Origin;
use gitprompt_insights::attention::PickUp;
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::stats::AggregateQuery;

use super::{Inner, Service};
use crate::error::ApiResult;
use crate::query::iso;

/// Prompts the tray lists.
pub const RECENT_PROMPTS: usize = 3;

/// Today (since local midnight by the app clock).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Today {
    pub prompts: i64,
    /// `None` when no tool reported a cost today (time is all there is, never a `$0`).
    pub cost_usd: Option<f64>,
}

/// One prompt you typed: its id and text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentPrompt {
    pub id: i64,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Glance {
    pub today: Today,
    /// As the Overview's Pick up: running first, then left, at most three.
    pub pick_up: Vec<PickUp>,
    /// Newest first, at most [`RECENT_PROMPTS`].
    pub recent: Vec<RecentPrompt>,
    /// Possible leaks not handled yet, whatever their age.
    pub open_leaks: usize,
}

impl Service {
    pub async fn glance(&self) -> ApiResult<Glance> {
        self.run(|inner| {
            let (set, _) = inner.indexed()?;
            let pick_up = inner.attention(&set, PromptFilter::default())?.pick_up;
            Ok(Glance { today: inner.today()?, pick_up, recent: inner.recent_prompts()?, open_leaks: inner.open_leaks()? })
        })
        .await
    }
}

impl Inner {
    fn today(&self) -> ApiResult<Today> {
        // `start_of_day` fails only at the ends of the supported range: then count from now.
        let now = self.local_now();
        let midnight = now.start_of_day().map_or_else(|_| now.timestamp(), |day| day.timestamp());
        let filter = PromptFilter { since: Some(iso(midnight)), ..PromptFilter::default() };
        let totals = self.store.aggregate(&AggregateQuery { filter, ..AggregateQuery::default() })?.totals;
        let cost_usd = (totals.api_requests > 0.0).then_some(totals.cost_usd);
        Ok(Today { prompts: totals.prompts, cost_usd })
    }

    fn recent_prompts(&self) -> ApiResult<Vec<RecentPrompt>> {
        let filter = PromptFilter { origin: vec![Origin::Typed], ..PromptFilter::default() };
        Ok(self
            .store
            .timeline(&filter, Some(RECENT_PROMPTS), true)?
            .into_iter()
            .filter_map(|prompt| Some(RecentPrompt { id: prompt.row.id, text: prompt.row.typed_text? }))
            .collect())
    }

    fn open_leaks(&self) -> ApiResult<usize> {
        Ok(self.leak_board(&PromptFilter::default())?.open.len())
    }
}
