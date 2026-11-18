//! Each key figure against the period before: a change only when there is something to compare.
#![allow(clippy::float_cmp, reason = "the values are small exact figures")]

use gitprompt_insights::delta::{Delta, Figures, KeyChanges};
use gitprompt_insights::redone::{RedonePart, RedoneWork};
use gitprompt_store::stats::Bucket;

fn figures(totals: &Bucket) -> Figures<'_> {
    Figures { totals, redone: None }
}

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
    let changes = KeyChanges::between(figures(&bucket(3, 2.5, 60.0)), figures(&Bucket::default()));
    for delta in [changes.prompts, changes.sessions, changes.cost_usd, changes.agent_time_s, changes.git_commits] {
        assert_eq!((delta.before, delta.change), (None, None));
    }
    assert_eq!(changes.cost_usd.now, 2.5);
}

#[test]
fn every_key_figure_compares_its_own_measure() {
    let changes = KeyChanges::between(figures(&bucket(4, 3.0, 90.0)), figures(&bucket(2, 2.0, 60.0)));
    assert_eq!(changes.prompts.change, Some(1.0));
    assert_eq!(changes.cost_usd.change, Some(0.5));
    assert_eq!(changes.agent_time_s.change, Some(0.5));
    assert_eq!(changes.git_commits, Delta { now: 0.0, before: Some(0.0), change: None });
}

fn redone(agent_time_s: f64, cost_usd: Option<f64>) -> RedoneWork {
    RedoneWork { total: RedonePart { turns: 1, agent_time_s, cost_usd }, ..RedoneWork::default() }
}

#[test]
fn redone_work_compares_only_when_asked_for_and_its_cost_only_where_reported() {
    let (now, before) = (bucket(4, 3.0, 90.0), bucket(2, 2.0, 60.0));
    assert_eq!(KeyChanges::between(figures(&now), figures(&before)).redone_agent_time_s, None, "not asked for");

    let (now_r, before_r) = (redone(120.0, None), redone(60.0, None));
    let changes = KeyChanges::between(Figures { totals: &now, redone: Some(&now_r) }, Figures { totals: &before, redone: Some(&before_r) });
    assert_eq!(changes.redone_agent_time_s, Some(Delta { now: 120.0, before: Some(60.0), change: Some(1.0) }));
    assert_eq!(changes.redone_cost_usd, None, "no tool reports cost: no cost figure, never $0");

    let costed = redone(120.0, Some(9.2));
    let changes =
        KeyChanges::between(Figures { totals: &now, redone: Some(&costed) }, Figures { totals: &before, redone: Some(&before_r) });
    assert_eq!(changes.redone_cost_usd, Some(Delta { now: 9.2, before: None, change: None }), "no cost before: nothing to compare");
}
