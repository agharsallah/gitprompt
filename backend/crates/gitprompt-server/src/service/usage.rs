//! Local usage counts: which parts of the dashboard are used, per day, on this machine only. The day
//! is the UTC date of the app clock's now.

use jiff::{SignedDuration, Timestamp};

use gitprompt_store::store::{USAGE_KEEP_DAYS, UsageCount};

use super::Service;
use crate::error::{ApiError, ApiResult};
use crate::model::{UsageDay, UsageIn, UsageOut};

/// Most hits one report may carry, and the most one hit may count: a page batches a few seconds of
/// clicks, never thousands.
const MAX_HITS: usize = 64;
const MAX_COUNT: u32 = 1000;
const MAX_ACTION_LEN: usize = 48;

/// A dotted lowercase name (`copy.prompt`, `panel.seen.rhythm`): a letter, then letters, digits,
/// `.`, `_` or `-`, at most 48 characters. Anything else could be a client stuffing data.
fn valid_action(action: &str) -> bool {
    let mut chars = action.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && action.len() <= MAX_ACTION_LEN
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

fn validate(body: &UsageIn) -> ApiResult<()> {
    let refuse = |why: String| Err(ApiError::Unprocessable(why));
    if body.hits.len() > MAX_HITS {
        return refuse(format!("hits: at most {MAX_HITS} per report"));
    }
    if let Some(hit) = body.hits.iter().find(|hit| !valid_action(&hit.action)) {
        return refuse(format!("action: not a dotted lowercase name of at most {MAX_ACTION_LEN} characters, got {:?}", hit.action));
    }
    if body.hits.iter().any(|hit| hit.n == 0 || hit.n > MAX_COUNT) {
        return refuse(format!("n: between 1 and {MAX_COUNT}"));
    }
    Ok(())
}

/// `YYYY-MM-DD` of `at`, UTC.
fn day(at: Timestamp) -> String {
    at.strftime("%Y-%m-%d").to_string()
}

/// The first day of the `days` days that end today (`now`'s day).
fn first_day(now: Timestamp, days: u32) -> String {
    let back = SignedDuration::from_hours(24 * i64::from(days.saturating_sub(1)));
    day(now.checked_sub(back).unwrap_or(Timestamp::MIN))
}

impl Service {
    /// Count what the page reports on today's row of each action, and drop the days no window
    /// reaches any more ([`USAGE_KEEP_DAYS`]).
    pub async fn record_usage(&self, body: UsageIn) -> ApiResult<()> {
        validate(&body)?;
        self.run(move |inner| {
            let now = inner.now();
            let (today, oldest) = (day(now), first_day(now, USAGE_KEEP_DAYS));
            inner.store.transaction(|store| {
                body.hits.iter().try_for_each(|hit| store.increment_usage(&hit.action, &today, u64::from(hit.n)))?;
                store.prune_usage(&oldest).map(drop)
            })?;
            Ok(())
        })
        .await
    }

    /// The counts of the last `days` days, today included.
    pub async fn usage(&self, days: u32) -> ApiResult<UsageOut> {
        self.run(move |inner| {
            let now = inner.now();
            let (since, until) = (first_day(now, days), day(now));
            let counts = inner.store.usage_between(&since, &until)?;
            Ok(UsageOut { since, until, counts: counts.into_iter().map(day_of).collect() })
        })
        .await
    }

    /// Forget every count.
    pub async fn clear_usage(&self) -> ApiResult<()> {
        self.run(|inner| Ok(inner.store.clear_usage()?)).await
    }
}

fn day_of(count: UsageCount) -> UsageDay {
    UsageDay { action: count.action, day: count.day, n: count.n }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_action_is_a_short_dotted_lowercase_name() {
        for good in ["copy.prompt", "panel.seen.rhythm", "reader.tab.events", "page.opened.stats", "a", "x-y_z.1"] {
            assert!(valid_action(good), "{good}");
        }
        for bad in ["", "Copy", "1abc", ".a", "a b", "a/b", "a\n", "é", &"a".repeat(49)] {
            assert!(!valid_action(bad), "{bad:?}");
        }
    }
}
