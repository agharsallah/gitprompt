//! Each key figure against the period before: a change only when there is something to compare.
#![allow(clippy::float_cmp, reason = "the values are small exact figures")]

use gitprompt_insights::delta::{Delta, KeyChanges};
use gitprompt_store::stats::Bucket;

fn bucket(prompts: i64, cost_usd: f64, agent_time_s: f64) -> Bucket {
    Bucket { prompts, sessions: prompts, cost_usd, agent_time_s, ..Bucket::default() }
}

#[test]
fn a_change_is_the_ratio_to_before_rounded() {
    assert_eq!(Delta::new(118.0, Some(100.0)).change, Some(0.18));
    assert_eq!(Delta::new(1.0, Some(3.0)).change, Some(-0.667));
    assert_eq!(Delta::new(0.0, Some(4.0)).change, Some(-1.0));
}

#[test]
fn without_a_before_or_with_zero_before_there_is_no_change() {
    assert_eq!(Delta::new(5.0, None), Delta { now: 5.0, before: None, change: None });
    assert_eq!(Delta::new(5.0, Some(0.0)), Delta { now: 5.0, before: Some(0.0), change: None });
    assert_eq!(Delta::new(0.0, Some(0.0)).change, None);
}

#[test]
fn a_period_before_without_prompts_is_no_before_for_any_figure() {
    let changes = KeyChanges::between(&bucket(3, 2.5, 60.0), &Bucket::default());
    for delta in [changes.prompts, changes.sessions, changes.cost_usd, changes.agent_time_s, changes.git_commits] {
        assert_eq!((delta.before, delta.change), (None, None));
    }
    assert_eq!(changes.cost_usd.now, 2.5);
}

#[test]
fn every_key_figure_compares_its_own_measure() {
    let changes = KeyChanges::between(&bucket(4, 3.0, 90.0), &bucket(2, 2.0, 60.0));
    assert_eq!(changes.prompts.change, Some(1.0));
    assert_eq!(changes.cost_usd.change, Some(0.5));
    assert_eq!(changes.agent_time_s.change, Some(0.5));
    assert_eq!(changes.git_commits, Delta { now: 0.0, before: Some(0.0), change: None });
}
