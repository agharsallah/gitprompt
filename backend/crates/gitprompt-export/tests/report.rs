//! The neutral report (export/report.py): finding its charts.

mod common;

use pretty_assertions::assert_eq;

use common::*;
use gitprompt_export::report::{Block, ChartKind, Paragraph};

fn sample() -> gitprompt_export::Report {
    report(vec![
        section(
            "One",
            vec![
                Block::Chart(chart("a", ChartKind::Bars, vec![bar("x", 1.0)])),
                Block::Paragraph(Paragraph { text: "p".into(), muted: false }),
                Block::Chart(chart("empty", ChartKind::Bars, vec![])),
            ],
        ),
        section("Two", vec![Block::Chart(chart("b", ChartKind::Columns, vec![bar("y", 2.0)]))]),
    ])
}

#[test]
fn charts_are_listed_in_section_then_block_order_including_empty_ones() {
    let report = sample();
    let ids: Vec<&str> = report.charts().into_iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["a", "empty", "b"]);
}

#[test]
fn a_chart_is_found_by_its_id() {
    assert_eq!(sample().chart("b"), Some(&chart("b", ChartKind::Columns, vec![bar("y", 2.0)])));
}

#[test]
fn an_unknown_chart_id_finds_nothing() {
    assert_eq!(sample().chart("nope"), None);
}

#[test]
fn a_report_without_charts_lists_none() {
    assert!(report(vec![section("Only text", vec![])]).charts().is_empty());
}

#[test]
fn chart_kinds_are_named_bars_and_columns() {
    assert_eq!([ChartKind::Bars.as_str(), ChartKind::Columns.as_str()], ["bars", "columns"]);
}
