//! The aggregate query: totals, groups, and the parts of each group.

use std::collections::HashMap;

use rusqlite::{Connection, Row};

use super::groups::{Excerpt, Group, group};
use super::{Aggregate, AggregateQuery, Bucket, Order, quantile};
use crate::decode::{self, text_at};
use crate::error::{DbResult, failure};
use crate::sql::filter::{conditions, followed};
use crate::sql::{Sql, int, json_list};

/// Measures of every bucket (`total()` is `sum()` giving 0.0 for no rows; `total(condition)`
/// counts the rows where it holds).
const MEASURES: &[(&str, &str)] = &[
    ("prompts", "count(*)"),
    ("sessions", "count(distinct i.session)"),
    ("api_requests", "total(i.api_requests)"),
    ("cost_usd", "total(i.cost_usd)"),
    ("input_tokens", "total(i.input_tokens)"),
    ("output_tokens", "total(i.output_tokens)"),
    ("cache_read_tokens", "total(i.cache_read_tokens)"),
    ("cache_creation_tokens", "total(i.cache_creation_tokens)"),
    ("api_time_s", "round(total(i.api_time_s), 1)"),
    ("api_errors", "total(i.api_errors)"),
    ("tool_calls", "total(i.tool_calls)"),
    ("tool_failures", "total(i.tool_failures)"),
    ("tool_time_s", "round(total(i.tool_time_s), 1)"),
    ("subagent_tool_calls", "total(i.subagent_tool_calls)"),
    ("subagents", "total(i.subagents)"),
    ("files_written", "total(i.files_written)"),
    ("git_commits", "total(i.git_commits)"),
    ("compactions", "total(i.compactions)"),
    ("denials", "total(i.denials)"),
    ("rejections", "total(i.rejections)"),
    ("ended", "count(i.response_s)"),
    ("failed", "total(i.ended_by = 'StopFailure')"),
    ("followed", concat!("total(", followed!(), ")")),
    ("with_code", "total(i.code_blocks > 0)"),
    ("with_url", "total(i.urls > 0)"),
    ("with_mention", "total(i.mentions > 0)"),
    ("with_secret", "total(i.secrets > 0)"),
    ("with_commit", "total(i.git_commits > 0)"),
    ("distinct_texts", "count(distinct i.text_hash)"),
    ("first_at", "min(i.received_at)"),
    ("last_at", "max(i.received_at)"),
];

/// Characters of typed text in an excerpt.
const EXCERPT_CHARS: usize = 200;

pub(super) fn aggregate(conn: &Connection, query: &AggregateQuery, running_since: &str) -> DbResult<Aggregate> {
    let filter = conditions(&query.filter, running_since)?;
    let totals = Sql::new(format!("select null key, null detail, {} from prompt_index i where ", measures(&[]))).push(filter.clone());
    let totals = decode::first(conn, totals.text(), totals.params(), bucket)?.unwrap_or_default();
    let Some(by) = query.group_by else {
        return Ok(Aggregate { totals, groups: Vec::new() });
    };

    let group = group(by, running_since);
    let order = order_by(&group, query.order);
    let inner = Sql::new("select ")
        .push(group.key.clone())
        .push(format!(" key, {} detail, {} from prompt_index i {} where ", group.detail, measures(&[&group]), group.join))
        .push(filter.clone())
        .push(format!(" group by 1 order by {order} limit "))
        .bind(int(query.limit));
    let select = with_excerpt(inner, group.excerpt, &order);
    let mut groups = decode::all(conn, select.text(), select.params(), bucket)?;

    if let Some(split_by) = query.split_by {
        add_parts(conn, &mut groups, &group, &self::group(split_by, running_since), filter)?;
    }
    Ok(Aggregate { totals, groups })
}

/// Give each group its parts: the same measures per value of the `split` grouping.
fn add_parts(conn: &Connection, groups: &mut [Bucket], group: &Group, split: &Group, filter: Sql) -> DbResult<()> {
    if groups.is_empty() {
        return Ok(());
    }
    let keys: Vec<&str> = groups.iter().filter_map(|g| g.key.as_deref()).collect();
    let nulls = groups.iter().any(|g| g.key.is_none());
    let kept = Sql::new("(")
        .push(group.key.clone())
        .push(" in (select value from json_each(")
        .bind(json_list(&keys)?)
        .push(")) or (")
        .push(group.key.clone())
        .push(" is null and ")
        .bind(i64::from(nulls))
        .push("))");
    let order = format!("parent, {}", order_by(split, None));
    let inner = Sql::new("select ")
        .push(group.key.clone())
        .push(" parent, ")
        .push(split.key.clone())
        .push(format!(
            " key, {} detail, {} from prompt_index i {} {} where ",
            split.detail,
            measures(&[group, split]),
            group.join,
            split.join
        ))
        .push(filter)
        .push(" and ")
        .push(kept)
        .push(format!(" group by 1, 2 order by {order}"));
    let select = with_excerpt(inner, split.excerpt, &order);
    let rows = decode::all(conn, select.text(), select.params(), |row| Ok((text_at(row, "parent")?, bucket(row)?)))?;

    let mut parts: HashMap<Option<String>, Vec<Bucket>> = HashMap::new();
    for (parent, part) in rows {
        parts.entry(parent).or_default().push(part);
    }
    for g in groups {
        g.parts = Some(parts.remove(&g.key).unwrap_or_default());
    }
    Ok(())
}

/// The select list of a bucket: every measure (with the groups' replacements), the response times
/// in one string (parsing it is faster than a row per prompt), and the first and last prompt ids.
fn measures(groups: &[&Group]) -> String {
    let mut measures: Vec<(&str, &str)> = MEASURES.to_vec();
    for &(name, sql) in groups.iter().flat_map(|g| g.measures) {
        match measures.iter_mut().find(|(existing, _)| *existing == name) {
            Some(slot) => slot.1 = sql,
            None => measures.push((name, sql)),
        }
    }
    let mut columns: Vec<String> = measures.iter().map(|(name, sql)| format!("{sql} {name}")).collect();
    columns.extend(["group_concat(i.response_s) times", "min(i.id) first_id", "max(i.id) last_id"].map(String::from));
    columns.join(", ")
}

fn order_by(group: &Group, order: Option<Order>) -> String {
    let order = order.unwrap_or(if group.by_key { Order::Key } else { Order::Prompts });
    let measure = match order {
        Order::Key => return "key".into(),
        Order::Prompts => "prompts",
        Order::Sessions => "sessions",
        Order::CostUsd => "cost_usd",
        Order::LastAt => "last_at",
    };
    format!("{measure} desc, key")
}

/// Add the excerpt of each bucket (an aggregate can't be used in a subquery of the same select, so
/// it is read in an outer one, over at most `limit` buckets).
fn with_excerpt(inner: Sql, excerpt: Option<Excerpt>, order: &str) -> Sql {
    let Some(excerpt) = excerpt else {
        return inner;
    };
    Sql::new(format!(
        "select q.*, coalesce(q.detail, (select substr(typed_text, 1, {EXCERPT_CHARS}) from prompt_fts where rowid = q.{})) excerpt from (",
        excerpt.id_column()
    ))
    .push(inner)
    .push(format!(") q order by {order}"))
}

fn bucket(row: &Row<'_>) -> DbResult<Bucket> {
    let has = |column: &str| row.as_ref().column_index(column).is_ok();
    let times = response_times(row.get("times")?)?;
    Ok(Bucket {
        key: text_at(row, "key")?,
        detail: text_at(row, if has("excerpt") { "excerpt" } else { "detail" })?,
        prompts: row.get("prompts")?,
        sessions: row.get("sessions")?,
        api_requests: row.get("api_requests")?,
        cost_usd: row.get("cost_usd")?,
        input_tokens: row.get("input_tokens")?,
        output_tokens: row.get("output_tokens")?,
        cache_read_tokens: row.get("cache_read_tokens")?,
        cache_creation_tokens: row.get("cache_creation_tokens")?,
        api_time_s: row.get("api_time_s")?,
        api_errors: row.get("api_errors")?,
        tool_calls: row.get("tool_calls")?,
        tool_failures: row.get("tool_failures")?,
        tool_time_s: row.get("tool_time_s")?,
        subagent_tool_calls: row.get("subagent_tool_calls")?,
        subagents: row.get("subagents")?,
        files_written: row.get("files_written")?,
        git_commits: row.get("git_commits")?,
        compactions: row.get("compactions")?,
        denials: row.get("denials")?,
        rejections: row.get("rejections")?,
        ended: row.get("ended")?,
        failed: row.get("failed")?,
        followed: row.get("followed")?,
        with_code: row.get("with_code")?,
        with_url: row.get("with_url")?,
        with_mention: row.get("with_mention")?,
        with_secret: row.get("with_secret")?,
        with_commit: row.get("with_commit")?,
        distinct_texts: row.get("distinct_texts")?,
        first_at: text_at(row, "first_at")?,
        last_at: text_at(row, "last_at")?,
        response_s_p50: quantile(&times, 0.5),
        response_s_p90: quantile(&times, 0.9),
        span_s: if has("span_s") { row.get("span_s")? } else { None },
        parts: None,
    })
}

/// The response times of a bucket (`group_concat` of them), sorted.
fn response_times(joined: Option<String>) -> DbResult<Vec<f64>> {
    let mut times = joined
        .iter()
        .flat_map(|joined| joined.split(','))
        .map(|t| t.trim().parse::<f64>().map_err(|_| failure(format!("stored response time is not a number: {t}"))))
        .collect::<DbResult<Vec<f64>>>()?;
    times.sort_by(f64::total_cmp);
    Ok(times)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_times_are_parsed_and_sorted() {
        assert_eq!(response_times(Some("30.0,10.0,100.5".into())).unwrap(), vec![10.0, 30.0, 100.5]);
        assert_eq!(response_times(None).unwrap(), Vec::<f64>::new());
    }

    #[test]
    fn a_group_measure_replaces_the_prompt_level_one_in_place() {
        let tool = group(crate::stats::GroupBy::ToolKind, "now");
        let select = measures(&[&tool]);
        assert!(select.contains("total(t.calls) tool_calls"));
        assert!(!select.contains("total(i.tool_calls)"));
    }
}
