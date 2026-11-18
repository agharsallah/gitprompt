//! `GET /api/stats` and its telemetry.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_insights::delta::Previous;
use gitprompt_store::filter::PromptFilter;
use gitprompt_store::stats::{Aggregate, GroupBy, Order};

/// Query of `GET /api/stats`: the prompt filters plus grouping. `limit` 1..=10000 (default 1000).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatsQuery {
    pub filter: PromptFilter,
    pub group_by: Option<GroupBy>,
    pub split_by: Option<GroupBy>,
    pub order: Option<Order>,
    pub limit: usize,
    /// `previous=true`: add the totals of the period of equal length before `since`.
    pub previous: bool,
}

impl Default for StatsQuery {
    fn default() -> Self {
        StatsQuery { filter: PromptFilter::default(), group_by: None, split_by: None, order: None, limit: 1000, previous: false }
    }
}

/// `GET /api/stats`. Group `detail` names a `source` group by its badge (spec `short_name`) and a
/// `surface` group by its label.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Stats {
    pub group_by: Option<GroupBy>,
    pub split_by: Option<GroupBy>,
    #[serde(flatten)]
    pub aggregate: Aggregate,
    /// With `previous=true` and a `since`: the period of equal length before, its totals and the
    /// key figures compared. Omitted otherwise (all time has no period before).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub previous: Option<Previous>,
}

pub use gitprompt_store::stats::Telemetry;
