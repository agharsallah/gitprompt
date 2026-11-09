//! Query-string parsing for the prompt filters and the routes that take them (port of
//! `api/filters.py`). Repeated keys build lists (`origin=typed&origin=slash_command`), as the
//! dashboard sends them; empty values are ignored; unknown keys are ignored.
//!
//! * `since` / `until`: ISO-8601; without an offset they are local time (the process's time zone).
//!   Normalized to UTC ISO-8601 with microseconds, as the store keeps instants.
//! * `weekday` 1..=7, `hour` 0..=23 (0 is a value), `limit` per route, `starred=true|false`.
//! * An invalid value of any known key (unknown origin, outcome, `has`, `group_by`...) is
//!   [`ApiError::Unprocessable`] naming the key.

use gitprompt_store::filter::PromptFilter;

use crate::error::ApiError;
use crate::model::{ExportQuery, PromptQuery, StatsQuery};

/// `(key, value)` pairs of a raw query string, percent-decoded, in order.
pub fn pairs(_query: Option<&str>) -> Vec<(String, String)> {
    todo!()
}

/// The prompt filters among the pairs.
pub fn prompt_filter(_pairs: &[(String, String)]) -> Result<PromptFilter, ApiError> {
    todo!()
}

/// Filters plus `limit` (1..=200, default 50): `/api/prompts`, `/api/search`.
pub fn prompt_query(_pairs: &[(String, String)]) -> Result<PromptQuery, ApiError> {
    todo!()
}

/// Filters plus `group_by`, `split_by` (needs `group_by`), `order`, `limit` (1..=10000).
pub fn stats_query(_pairs: &[(String, String)]) -> Result<StatsQuery, ApiError> {
    todo!()
}

/// The export options among the pairs (the filters are read separately).
pub fn export_query(_pairs: &[(String, String)]) -> Result<ExportQuery, ApiError> {
    todo!()
}

/// `dry_run` of the write routes, with the route's default.
pub fn dry_run(_pairs: &[(String, String)], _default: bool) -> Result<bool, ApiError> {
    todo!()
}
