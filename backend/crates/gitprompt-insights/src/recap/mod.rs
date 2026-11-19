//! The weekly recap: once a week, one notification with what the week cost, what was redone, what
//! was committed and the one thing worth a look. Pure: the daemon reads the week's figures and the
//! attention feed, and passes "now" and the zone to [`schedule`] to know when to send it.

mod schedule;

pub use schedule::{last_recap, next_recap, week_before};

use gitprompt_core::numbers::{duration, money};
use gitprompt_core::preferences::RangeId;
use gitprompt_store::stats::Bucket;

use crate::attention::Attention;
use crate::notify::{Kind, Notification};
use crate::redone::RedoneWork;
use crate::target::Target;

/// The recap's title.
pub const TITLE: &str = "Your week";

/// What a week's recap says, read from its figures.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Week {
    pub prompts: i64,
    /// What it cost; `None` when no tool used that week reports cost (never a misleading $0).
    pub cost_usd: Option<f64>,
    /// Agent time of the turns you redid, seconds.
    pub redone_s: f64,
    /// Commits made during the turns.
    pub commits: i64,
}

impl Week {
    /// The week's figures from its totals and redone work; `reports_cost` says whether a tool used
    /// that week reports cost.
    pub fn new(totals: &Bucket, redone: Option<&RedoneWork>, reports_cost: bool) -> Week {
        Week {
            prompts: totals.prompts,
            cost_usd: reports_cost.then_some(totals.cost_usd),
            redone_s: redone.map_or(0.0, |r| r.total.agent_time_s),
            commits: gitprompt_core::numbers::whole(totals.git_commits),
        }
    }
}

/// The recap of `week`, pointing at the top Worth a look item of `attention`: `None` for a week
/// with no prompt. It opens the Overview on the last seven days.
pub fn recap(week: &Week, attention: &Attention) -> Option<Notification> {
    if week.prompts <= 0 {
        return None;
    }
    let pointer = attention.worth_a_look.first().map(|item| format!(" Worth a look: {}.", item.text.trim_end_matches('.')));
    let body = format!("This week: {}.{}", facts(week).join(" · "), pointer.unwrap_or_default());
    Some(Notification { kind: Kind::WeeklyRecap, title: TITLE.to_owned(), body, target: Target::Overview { range: Some(RangeId::Week) } })
}

/// Spend (where reported), redone agent time, commits.
fn facts(week: &Week) -> Vec<String> {
    let spend = week.cost_usd.map(money);
    let redone = if week.redone_s > 0.0 { format!("{} redone", duration(week.redone_s)) } else { "nothing redone".to_owned() };
    let commits = match week.commits {
        0 => "no commits".to_owned(),
        1 => "1 commit".to_owned(),
        n => format!("{n} commits"),
    };
    spend.into_iter().chain([redone, commits]).collect()
}
