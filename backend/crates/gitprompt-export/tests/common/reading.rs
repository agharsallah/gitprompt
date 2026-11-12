//! Reading a built report back in tests: its sections, figures, charts, tables and row keys.

use gitprompt_export::Row;
use gitprompt_export::report::{Block, Chart, Figure, Report, Section, Table};

pub fn figures(section: &Section) -> Vec<(String, String)> {
    match &section.blocks[0] {
        Block::Figures(f) => f.items.iter().map(|Figure { label, value }| (label.clone(), value.clone())).collect(),
        other => panic!("not figures: {other:?}"),
    }
}

pub fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

pub fn chart<'a>(report: &'a Report, id: &str) -> &'a Chart {
    report.chart(id).unwrap_or_else(|| panic!("no chart {id}"))
}

pub fn bars_of(chart: &Chart) -> Vec<(String, String)> {
    chart.items.iter().map(|b| (b.label.clone(), b.display.clone())).collect()
}

pub fn section<'a>(report: &'a Report, title: &str) -> &'a Section {
    report.sections.iter().find(|s| s.title == title).unwrap_or_else(|| panic!("no section {title}"))
}

pub fn titles(report: &Report) -> Vec<&str> {
    report.sections.iter().map(|s| s.title.as_str()).collect()
}

pub fn keys(row: &Row) -> Vec<&str> {
    row.keys().map(String::as_str).collect()
}

pub fn table(section: &Section) -> &Table {
    section.blocks.iter().find_map(|b| if let Block::Table(t) = b { Some(t) } else { None }).expect("a table")
}
