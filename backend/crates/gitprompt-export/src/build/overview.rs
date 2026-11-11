//! The Overview: the dashboard's statistics as a report and long-format rows.

use indexmap::IndexMap;

use gitprompt_core::Json;
use gitprompt_store::stats::Bucket;

use super::read::count;
use super::{BUCKET_COLUMNS, OVERVIEW_GROUPS, fields, figure, pick, present, report, section};
use super::{bars, describe_filters, duration, integer, money, origin_label, outcome_chart, outcome_label, when};
use crate::input::{OverviewExport, OverviewGroups};
use crate::redact::Options;
use crate::report::{Bar, Block, Chart, ChartKind, Figures, Level, Report, Section, Table};
use crate::{Kind, Row, Zone};

/// A bucket with the name it is shown by.
type Named<'a> = (String, &'a Bucket);

/// The Overview: summary and activity, prompt types, models and cost, agentic work, projects. Rows:
/// long-format statistics, a `total` row then one per bucket of each of [`OVERVIEW_GROUPS`].
pub fn overview(data: &OverviewExport, options: &Options, zone: Zone) -> (Report, Vec<Row>) {
    let by: Vec<(&str, Vec<Named>)> = OVERVIEW_GROUPS.iter().map(|&name| (name, named(name, group(&data.groups, name)))).collect();
    let get = |name: &str| by.iter().find(|(n, _)| *n == name).map(|(_, buckets)| buckets.as_slice()).unwrap_or_default();
    let sections = vec![
        section("Summary", vec![figures(&data.totals), Block::Chart(activity(get("day"), zone))], String::new(), Level::Two),
        section(
            "Prompt types",
            vec![
                Block::Chart(bars("origins", "Who wrote the prompts", values(get("origin"), false, prompts), &count, "")),
                Block::Chart(bars("commands", "Slash commands", values(get("command"), true, prompts), &count, "")),
            ],
            String::new(),
            Level::Two,
        ),
        section(
            "Models and cost",
            vec![
                Block::Chart(bars("models", "Prompts by model", values(get("model"), true, prompts), &count, "")),
                Block::Chart(bars("cost", "Cost by model", values(get("model"), true, |b| b.cost_usd), &|v| money(Some(v)), "")),
            ],
            String::new(),
            Level::Two,
        ),
        section(
            "Agentic work",
            vec![
                Block::Chart(bars("tool-kinds", "Tool calls by kind", values(get("tool_kind"), true, |b| b.tool_calls), &count, "")),
                Block::Chart(outcome_chart(&outcomes(get("outcome")))),
            ],
            String::new(),
            Level::Two,
        ),
        projects(get("repo")),
    ];
    let mut rows = vec![total_row(&data.totals)];
    rows.extend(by.iter().flat_map(|(name, buckets)| buckets.iter().map(|(shown, b)| bucket_row(name, shown, b))));
    let subtitle = describe_filters(&data.filters, zone);
    (report(Kind::Overview, &data.generated_at, "Overview".to_string(), subtitle, sections, *options, zone), rows)
}

fn group<'a>(groups: &'a OverviewGroups, name: &str) -> &'a [Bucket] {
    match name {
        "day" => &groups.day,
        "origin" => &groups.origin,
        "model" => &groups.model,
        "tool_kind" => &groups.tool_kind,
        "outcome" => &groups.outcome,
        "repo" => &groups.repo,
        "command" => &groups.command,
        _ => &[],
    }
}

/// Each bucket with its name: an origin or outcome as read, a project by its remote else its
/// folder, `None` for the bucket without a key.
fn named<'a>(group: &str, buckets: &'a [Bucket]) -> Vec<Named<'a>> {
    buckets
        .iter()
        .map(|b| {
            let name = match (b.key.as_deref(), group) {
                (None, _) => "None".to_string(),
                (Some(key), "origin") => origin_label(key),
                (Some(key), "outcome") => outcome_label(key),
                (Some(key), "repo") => present(b.detail.as_deref()).unwrap_or(key).to_string(),
                (Some(key), _) => key.to_string(),
            };
            (name, b)
        })
        .collect()
}

fn prompts(bucket: &Bucket) -> f64 {
    bucket.prompts as f64
}

/// Chart values of named buckets; `keyed`: only buckets with a (non-empty) key.
fn values(buckets: &[Named], keyed: bool, value: impl Fn(&Bucket) -> f64) -> Vec<(String, f64)> {
    buckets.iter().filter(|(_, b)| !keyed || present(b.key.as_deref()).is_some()).map(|(name, b)| (name.clone(), value(b))).collect()
}

fn outcomes(buckets: &[Named]) -> IndexMap<String, i64> {
    buckets.iter().filter_map(|(_, b)| present(b.key.as_deref()).map(|key| (key.to_string(), b.prompts))).collect()
}

/// Headline figures; cost and tokens only when some tool reported them (left out, not zero).
fn figures(t: &Bucket) -> Block {
    let mut items = vec![figure("Prompts", &integer(Some(t.prompts as f64))), figure("Sessions", &integer(Some(t.sessions as f64)))];
    if t.cost_usd != 0.0 {
        items.push(figure("Cost", &money(Some(t.cost_usd))));
    }
    if t.api_requests != 0.0 {
        items.push(figure("Tokens", &integer(Some(t.input_tokens + t.output_tokens))));
    }
    items.extend([
        figure("Tool calls", &integer(Some(t.tool_calls))),
        figure("Failed turns", &integer(Some(t.failed))),
        figure("Median response", &duration(t.response_s_p50)),
        figure("Commits by the agent", &integer(Some(t.git_commits))),
    ]);
    Block::Figures(Figures { items })
}

fn activity(days: &[Named], zone: Zone) -> Chart {
    let items = days
        .iter()
        .map(|(_, b)| Bar { label: when(b.key.as_deref(), true, zone), value: prompts(b), display: integer(Some(prompts(b))) })
        .collect();
    Chart { id: "activity".to_string(), title: "Prompts per day".to_string(), kind: ChartKind::Columns, items, caption: String::new() }
}

fn projects(repos: &[Named]) -> Section {
    let rows = repos
        .iter()
        .map(|(name, b)| {
            vec![
                name.clone(),
                integer(Some(b.prompts as f64)),
                integer(Some(b.sessions as f64)),
                money(Some(b.cost_usd)),
                integer(Some(b.tool_calls)),
            ]
        })
        .collect();
    let columns = ["Project", "Prompts", "Sessions", "Cost", "Tool calls"].map(String::from).to_vec();
    let table = Table { columns, rows, numeric: vec![false, true, true, true, true] };
    section("Projects", vec![Block::Table(table)], String::new(), Level::Two)
}

fn total_row(totals: &Bucket) -> Row {
    let mut row = Row::new();
    row.insert("grouping".to_string(), Json::from("total"));
    row.insert("key".to_string(), Json::Null);
    row.extend(measures(totals));
    row
}

fn bucket_row(grouping: &str, name: &str, bucket: &Bucket) -> Row {
    let mut row = Row::new();
    row.insert("grouping".to_string(), Json::from(grouping));
    row.insert("key".to_string(), bucket.key.clone().map_or(Json::Null, Json::from));
    row.insert("name".to_string(), Json::from(name));
    row.extend(measures(bucket));
    row
}

/// A bucket's [`BUCKET_COLUMNS`], whole numbers as integers (as SQL sums read: `7`, not `7.0`).
fn measures(bucket: &Bucket) -> Row {
    pick(&fields(bucket), BUCKET_COLUMNS).into_iter().map(|(key, value)| (key, whole(value))).collect()
}

/// A float that holds a whole number (below 2^53, so exactly) as an integer; anything else as is.
fn whole(value: Json) -> Json {
    const EXACT: f64 = 9_007_199_254_740_992.0;
    match value.as_f64() {
        #[expect(clippy::cast_possible_truncation, reason = "a whole number below 2^53 converts exactly")]
        Some(n) if value.is_f64() && n.fract() == 0.0 && n.abs() < EXACT => Json::from(n as i64),
        _ => value,
    }
}
