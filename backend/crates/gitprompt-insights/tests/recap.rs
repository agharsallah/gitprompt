//! The weekly recap: its words (four facts and one pointer) and when it is due, in local time
//! across week edges, time zones, a changed preference and daylight saving changes.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

mod common;

use jiff::Timestamp;
use jiff::tz::Offset;
use pretty_assertions::assert_eq;

use common::Berlin2026;
use gitprompt_core::preferences::{Preferences, RangeId, Weekday};
use gitprompt_insights::attention::{Action, Attention, Item, Kind as WorthKind};
use gitprompt_insights::notify::{Kind, Notification};
use gitprompt_insights::recap::{TITLE, Week, last_recap, next_recap, recap, week_before};
use gitprompt_insights::redone::{RedonePart, RedoneWork};
use gitprompt_insights::target::Target;
use gitprompt_store::stats::Bucket;

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn prefs(weekday: Weekday, hour: u8) -> Preferences {
    Preferences { recap_weekday: weekday, recap_hour: hour, ..Preferences::default() }
}

fn utc() -> Offset {
    Offset::UTC
}

// --- when it is due ---

#[test]
fn by_default_it_is_due_on_monday_at_nine() {
    let now = at("2026-11-18T08:00:00Z"); // a Wednesday
    let default = Preferences::default();
    assert_eq!(next_recap(now, &default, &utc()), at("2026-11-23T09:00:00Z"));
    assert_eq!(last_recap(now, &default, &utc()), at("2026-11-16T09:00:00Z"));
}

#[test]
fn the_instant_itself_is_the_last_one_and_the_next_is_a_week_later() {
    let monday = prefs(Weekday::Monday, 9);
    let due = at("2026-11-16T09:00:00Z");
    assert_eq!(last_recap(due, &monday, &utc()), due);
    assert_eq!(next_recap(due, &monday, &utc()), at("2026-11-23T09:00:00Z"));
    let just_before = at("2026-11-16T08:59:59Z");
    assert_eq!(next_recap(just_before, &monday, &utc()), due);
    assert_eq!(last_recap(just_before, &monday, &utc()), at("2026-11-09T09:00:00Z"));
}

#[test]
fn a_week_edge_across_the_new_year_is_a_plain_week() {
    let now = at("2026-12-31T12:00:00Z"); // a Thursday
    assert_eq!(next_recap(now, &Preferences::default(), &utc()), at("2027-01-04T09:00:00Z"));
    assert_eq!(last_recap(now, &Preferences::default(), &utc()), at("2026-12-28T09:00:00Z"));
}

#[test]
fn the_weekday_and_hour_are_local() {
    let plus_two = Offset::from_seconds(2 * 3600).unwrap();
    // Sunday 23:00 UTC is already Monday 01:00 at +02:00: today's 09:00 is next.
    let now = at("2026-11-22T23:00:00Z");
    assert_eq!(next_recap(now, &Preferences::default(), &plus_two), at("2026-11-23T07:00:00Z"));
    let minus_five = Offset::from_seconds(-5 * 3600).unwrap();
    // Monday 10:00 UTC is still Monday 05:00 at −05:00: 09:00 there is 14:00 UTC.
    assert_eq!(next_recap(at("2026-11-23T10:00:00Z"), &Preferences::default(), &minus_five), at("2026-11-23T14:00:00Z"));
}

#[test]
fn a_changed_preference_moves_the_next_instant() {
    let now = at("2026-11-18T08:00:00Z"); // a Wednesday
    assert_eq!(next_recap(now, &prefs(Weekday::Friday, 18), &utc()), at("2026-11-20T18:00:00Z"));
    assert_eq!(next_recap(now, &prefs(Weekday::Wednesday, 7), &utc()), at("2026-11-25T07:00:00Z"), "today's hour has passed");
    assert_eq!(next_recap(now, &prefs(Weekday::Wednesday, 23), &utc()), at("2026-11-18T23:00:00Z"));
    assert_eq!(last_recap(now, &prefs(Weekday::Sunday, 0), &utc()), at("2026-11-15T00:00:00Z"));
}

#[test]
fn across_a_daylight_saving_change_the_local_hour_holds() {
    let monday = Preferences::default();
    // Spring forward (2026-03-29): Monday 09:00 is 08:00 UTC before, 07:00 UTC after.
    assert_eq!(next_recap(at("2026-03-28T12:00:00Z"), &monday, &Berlin2026), at("2026-03-30T07:00:00Z"));
    assert_eq!(last_recap(at("2026-03-28T12:00:00Z"), &monday, &Berlin2026), at("2026-03-23T08:00:00Z"));
    // Fall back (2026-10-25): 07:00 UTC before, 08:00 UTC after.
    assert_eq!(next_recap(at("2026-10-24T12:00:00Z"), &monday, &Berlin2026), at("2026-10-26T08:00:00Z"));
    // The week a recap sums up is seven local days: 167 hours across the spring change.
    assert_eq!(week_before(at("2026-03-30T07:00:00Z"), &Berlin2026), at("2026-03-23T08:00:00Z"));
}

#[test]
fn an_hour_skipped_or_repeated_by_the_change_is_still_one_instant() {
    let two_on_sunday = prefs(Weekday::Sunday, 2);
    // 02:00 never happens on 2026-03-29 in Berlin: the recap comes just after the gap (03:00 CEST).
    assert_eq!(next_recap(at("2026-03-28T12:00:00Z"), &two_on_sunday, &Berlin2026), at("2026-03-29T01:00:00Z"));
    // 02:00 happens twice on 2026-10-25: the first one.
    assert_eq!(next_recap(at("2026-10-24T12:00:00Z"), &two_on_sunday, &Berlin2026), at("2026-10-25T00:00:00Z"));
}

// --- what it says ---

fn week(cost_usd: Option<f64>, redone_s: f64, commits: i64) -> Week {
    Week { prompts: 120, cost_usd, redone_s, commits }
}

fn worth(text: &str) -> Attention {
    let item = Item {
        kind: WorthKind::RedoneTurn,
        text: text.into(),
        action: Action::OpenTurn { prompt_id: 7 },
        target: Target::Prompt { id: 7 },
    };
    Attention { pick_up: Vec::new(), worth_a_look: vec![item], nothing_needs_you: false }
}

fn calm() -> Attention {
    Attention { pick_up: Vec::new(), worth_a_look: Vec::new(), nothing_needs_you: true }
}

#[test]
fn the_recap_says_four_facts_and_one_pointer_and_opens_the_overview_on_the_week() {
    let note = recap(&week(Some(42.0), 4200.0, 23), &worth("Costliest redone turn: $4.10")).unwrap();
    let expected = Notification {
        kind: Kind::WeeklyRecap,
        title: TITLE.into(),
        body: "This week: $42.00 · 1 h 10 min redone · 23 commits. Worth a look: Costliest redone turn: $4.10.".into(),
        target: Target::Overview { range: Some(RangeId::Week) },
    };
    assert_eq!(note, expected);
}

#[test]
fn spend_is_left_out_when_no_tool_reports_cost_and_nothing_redone_is_said() {
    let note = recap(&week(None, 0.0, 1), &calm()).unwrap();
    assert_eq!(note.body, "This week: nothing redone · 1 commit.");
    let free = recap(&week(Some(0.0), 0.0, 0), &calm()).unwrap();
    assert_eq!(free.body, "This week: $0.00 · nothing redone · no commits.", "a reported $0 is a fact");
}

#[test]
fn a_pointer_ending_in_a_full_stop_gets_only_one() {
    let note = recap(&week(None, 45.0, 2), &worth("A possible leak.")).unwrap();
    assert_eq!(note.body, "This week: 45 s redone · 2 commits. Worth a look: A possible leak.");
}

#[test]
fn a_week_with_no_prompt_sends_nothing() {
    let empty = Week { prompts: 0, ..week(Some(3.0), 10.0, 1) };
    assert_eq!(recap(&empty, &worth("anything")), None);
}

#[test]
fn the_week_is_read_from_its_totals_and_redone_work() {
    let totals = Bucket { prompts: 9, cost_usd: 12.5, git_commits: 3.0, ..Bucket::default() };
    let redone = RedoneWork { total: RedonePart { turns: 2, agent_time_s: 600.0, cost_usd: None }, ..RedoneWork::default() };
    assert_eq!(Week::new(&totals, Some(&redone), true), Week { prompts: 9, cost_usd: Some(12.5), redone_s: 600.0, commits: 3 });
    assert_eq!(Week::new(&totals, None, false), Week { prompts: 9, cost_usd: None, redone_s: 0.0, commits: 3 });
}
