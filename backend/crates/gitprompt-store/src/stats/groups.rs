//! Every [`GroupBy`] as data: its key, the join that gives one row per value (multi-valued
//! groupings), the measures it replaces, its order and detail. To add a grouping, add a variant to
//! `GroupBy` and one line to [`group`].

use crate::sql::Sql;
use crate::sql::filter::{buckets, outcome, size_bucket};
use crate::stats::{GroupBy, SESSION_LENGTHS};

pub(super) struct Group {
    /// SQL of the group's key (text, or null).
    pub key: Sql,
    /// Joins needed by the key and measures.
    pub join: &'static str,
    /// A prompt counts in every group it touched (one row per value after `join`).
    pub multi_valued: bool,
    /// Measures that replace (or add to) the prompt-level ones.
    pub measures: &'static [(&'static str, &'static str)],
    /// Ordered by key (time and size buckets) rather than by prompt count.
    pub by_key: bool,
    /// SQL of one more fact about the group: a name, a kind.
    pub detail: &'static str,
    /// Without a detail, the start of the group's first or last typed text.
    pub excerpt: Option<Excerpt>,
}

#[derive(Clone, Copy)]
pub(super) enum Excerpt {
    First,
    Last,
}

impl Excerpt {
    /// The column of a bucket holding the prompt to excerpt.
    pub(super) fn id_column(self) -> &'static str {
        match self {
            Excerpt::First => "first_id",
            Excerpt::Last => "last_id",
        }
    }
}

/// The grouping table.
pub(super) fn group(by: GroupBy, running_since: &str) -> Group {
    match by {
        GroupBy::Day => Group::on("i.day").by_key(),
        GroupBy::DayHour => Group::on("i.day || ' ' || printf('%02d', i.hour)").by_key(),
        GroupBy::Hour => Group::on("printf('%02d', i.hour)").by_key(),
        GroupBy::Weekday => Group::on("cast(i.weekday as text)").by_key(),
        GroupBy::WeekdayHour => Group::on("i.weekday || '-' || printf('%02d', i.hour)").by_key(),
        GroupBy::Origin => Group::on("i.origin"),
        GroupBy::Source => Group::on("i.source"),
        GroupBy::Surface => Group::on("i.surface"),
        GroupBy::Session => Group::on("i.session").measures(SESSION_SPAN).detail("max(i.title)").excerpt(Excerpt::First),
        GroupBy::SessionLength => Group::on(buckets("ss.span", SESSION_LENGTHS)).join(SESSIONS).by_key(),
        GroupBy::Repo => Group::on("i.repo").detail("max(i.repo_name)"),
        GroupBy::Branch => Group::on("i.branch"),
        GroupBy::Command => Group::on("i.slash_command"),
        GroupBy::Size => Group::on(size_bucket()).by_key(),
        GroupBy::Text => Group::on("i.text_hash").excerpt(Excerpt::Last),
        GroupBy::Outcome => Group::on(outcome(running_since)),
        GroupBy::Model => Group::on("m.model").per_value(MODEL, MODEL_USAGE),
        GroupBy::Tool => Group::on("t.tool").per_value(TOOL, TOOL_CALLS).detail("max(t.kind)"),
        GroupBy::ToolKind => Group::on("t.kind").per_value(TOOL, TOOL_CALLS),
        GroupBy::Work => Group::on("w.work").per_value(WORK_KIND, WORK_USAGE),
        GroupBy::QuerySource => Group::on("w.query_source").per_value(WORK, WORK_USAGE).detail("max(w.work)"),
    }
}

impl Group {
    fn on(key: impl Into<Sql>) -> Group {
        Group { key: key.into(), join: "", multi_valued: false, measures: &[], by_key: false, detail: "null", excerpt: None }
    }

    fn by_key(self) -> Group {
        Group { by_key: true, ..self }
    }

    fn join(self, join: &'static str) -> Group {
        Group { join, ..self }
    }

    /// One row per value of the prompt; `measures` split the prompt's totals exactly between them.
    fn per_value(self, join: &'static str, measures: &'static [(&'static str, &'static str)]) -> Group {
        Group { join, measures, multi_valued: true, ..self }
    }

    fn measures(self, measures: &'static [(&'static str, &'static str)]) -> Group {
        Group { measures, ..self }
    }

    fn detail(self, detail: &'static str) -> Group {
        Group { detail, ..self }
    }

    fn excerpt(self, excerpt: Excerpt) -> Group {
        Group { excerpt: Some(excerpt), ..self }
    }
}

const MODEL: &str = "join prompt_index_model m on m.id = i.id";
const TOOL: &str = "join prompt_index_tool t on t.id = i.id";
const WORK: &str = "join prompt_index_work w on w.id = i.id";
/// One row per prompt and kind of work (several query sources map to one kind).
const WORK_KIND: &str = "join (select id, work, total(api_requests) api_requests, total(cost_usd) cost_usd, \
     total(input_tokens) input_tokens, total(output_tokens) output_tokens, total(cache_read_tokens) cache_read_tokens, \
     total(cache_creation_tokens) cache_creation_tokens from prompt_index_work group by id, work) w on w.id = i.id";

/// The usage measures of a per-value table aliased `$alias`.
macro_rules! usage_of {
    ($alias:literal) => {
        &[
            ("api_requests", concat!("total(", $alias, ".api_requests)")),
            ("cost_usd", concat!("total(", $alias, ".cost_usd)")),
            ("input_tokens", concat!("total(", $alias, ".input_tokens)")),
            ("output_tokens", concat!("total(", $alias, ".output_tokens)")),
            ("cache_read_tokens", concat!("total(", $alias, ".cache_read_tokens)")),
            ("cache_creation_tokens", concat!("total(", $alias, ".cache_creation_tokens)")),
        ]
    };
}

const MODEL_USAGE: &[(&str, &str)] = usage_of!("m");
const WORK_USAGE: &[(&str, &str)] = usage_of!("w");
const TOOL_CALLS: &[(&str, &str)] = &[("tool_calls", "total(t.calls)")];

/// Seconds from a session's first prompt to the end of its last turn, over its prompts `i`.
macro_rules! span {
    () => {
        "(max((julianday(i.received_at) * 86400.0) + coalesce(i.response_s, 0)) - min((julianday(i.received_at) * 86400.0)))"
    };
}

const SESSION_SPAN: &[(&str, &str)] = &[("span_s", concat!("round(", span!(), ", 1)"))];
/// Each prompt's session span over all its prompts, whatever the filters.
const SESSIONS: &str = concat!(
    "join (select i.session sid, ",
    span!(),
    " span from prompt_index i where i.session is not null group by i.session) ss on ss.sid = i.session"
);
