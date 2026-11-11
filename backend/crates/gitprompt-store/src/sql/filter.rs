//! A [`PromptFilter`] as SQL conditions on the prompt index `i`, plus the values derived when a query
//! runs (a later prompt, the outcome, the size bucket). Port of storage/store.py FILTERS/where.
//! Empty values are no filter; `hour`/`weekday` 0 are values.

use rusqlite::types::Value;
use serde::Serialize;

use crate::error::DbResult;
use crate::filter::{Has, PromptFilter, SIZES};
use crate::sql::{Sql, in_list};

/// FTS5's trigram tokenizer matches words of at least 3 characters.
const TRIGRAM: usize = 3;

/// A later prompt in the same session (any origin): the user (or the tool) went on after this turn.
/// A macro so measures and conditions can `concat!` it.
macro_rules! followed {
    () => {
        "exists (select 1 from prompt_index n where n.session = i.session and n.id > i.id)"
    };
}
pub(crate) use followed;

pub(crate) const FOLLOWED: &str = followed!();

/// How a turn ended (`Outcome`'s names). `running_since`: `received_at` one hour before now.
pub(crate) fn outcome(running_since: &str) -> Sql {
    Sql::new(
        "(case when i.ended_by = 'StopFailure' then 'failed' when i.ended_by = 'Interrupt' then 'interrupted' \
         when i.ended_by is not null then 'done' when ",
    )
    .push(FOLLOWED)
    .push(" then 'superseded' when i.received_at >= ")
    .bind(running_since.to_owned())
    .push(" then 'running' else 'open' end)")
}

/// SQL putting `value` in the bucket of the largest lower bound it reaches, as a zero-padded key (so
/// keys sort as numbers): bounds (0, 20, 80) give '00000', '00020', '00080'.
pub(crate) fn buckets(value: &str, bounds: &[u32]) -> String {
    let cases: Vec<String> = bounds
        .windows(2)
        .filter_map(|w| match w {
            [lo, hi] => Some(format!("when {value} < {hi} then '{lo:05}'")),
            _ => None,
        })
        .collect();
    let last = bounds.last().copied().unwrap_or(0);
    format!("(case {} else '{last:05}' end)", cases.join(" "))
}

/// The typed-character bucket of a prompt (a key of [`SIZES`]).
pub(crate) fn size_bucket() -> String {
    buckets("i.chars", SIZES)
}

/// What a prompt or its turn contains.
fn has(condition: Has) -> &'static str {
    match condition {
        Has::Code => "i.code_blocks > 0",
        Has::Url => "i.urls > 0",
        Has::Mention => "i.mentions > 0",
        Has::Secret => "i.secrets > 0",
        Has::Commit => "i.git_commits > 0",
        Has::Subagent => "i.subagents > 0",
        Has::ToolFailure => "i.tool_failures > 0",
        Has::ApiError => "i.api_errors > 0",
        Has::Denial => "(i.denials > 0 or i.rejections > 0)",
        Has::Compaction => "i.compactions > 0",
        Has::NoFollowUp => concat!("not ", followed!()),
    }
}

/// The conditions of a filter (every one must hold), `1=1` for none.
pub(crate) fn conditions(f: &PromptFilter, running_since: &str) -> DbResult<Sql> {
    let tool_kind = f.tool_kind.as_ref().map(name).transpose()?;
    let conditions = [
        text(f.q.as_deref()).map(words),
        listed("i.origin", &f.origin)?,
        listed("i.surface", &f.surface)?,
        listed("model", &f.model)?.map(|c| of_turn("prompt_index_model", c)),
        text(f.source.as_deref()).map(|v| equals("i.source", v)),
        listed("i.session", &f.session)?,
        text(f.repo.as_deref()).map(|v| equals("i.repo", v)),
        text(f.since.as_deref()).map(|v| Sql::new("i.received_at >= ").bind(v.to_owned())),
        text(f.until.as_deref()).map(|v| Sql::new("i.received_at < ").bind(v.to_owned())),
        f.weekday.map(|v| equals("i.weekday", v)),
        f.hour.map(|v| equals("i.hour", v)),
        text(f.command.as_deref()).map(|v| equals("i.slash_command", v)),
        text(f.tool.as_deref()).map(|v| of_turn("prompt_index_tool", equals("tool", v))),
        tool_kind.map(|v| of_turn("prompt_index_tool", equals("kind", v))),
        text(f.query_source.as_deref()).map(|v| of_turn("prompt_index_work", equals("query_source", v))),
        listed(outcome(running_since), &f.outcome)?,
        text(f.size.as_deref()).map(|v| Sql::new(size_bucket()).push(" = ").bind(v.to_owned())),
        (!f.has.is_empty()).then(|| Sql::all(f.has.iter().map(|h| Sql::new(has(*h))))),
        f.before.map(|v| Sql::new("i.id < ").bind(v)),
        text(f.tag.as_deref()).map(|v| {
            Sql::new("i.id in (select a.id from annotation a, json_each(a.data, '$.tags') t where t.value = ").bind(v.to_owned()).push(")")
        }),
        // Ported as is: `starred: false` is no filter (Python treats False as an empty value).
        (f.starred == Some(true)).then(|| Sql::new("i.id in (select id from annotation where json_extract(data, '$.starred') = 1)")),
    ];
    Ok(Sql::all(conditions.into_iter().flatten()))
}

/// Every word must appear in the typed text, anywhere, ignoring case. Words of 3+ characters go
/// through the full-text index (as quoted phrases: FTS syntax is plain text); shorter ones are
/// matched with LIKE on the rows that have a typed text (`%`, `_` and `\` escaped).
fn words(q: &str) -> Sql {
    let (long, short): (Vec<&str>, Vec<&str>) = q.split_whitespace().partition(|w| w.chars().count() >= TRIGRAM);
    let phrases = (!long.is_empty()).then(|| {
        let query: Vec<String> = long.iter().map(|w| format!("\"{}\"", w.replace('"', "\"\""))).collect();
        Sql::new("i.id in (select rowid from prompt_fts where prompt_fts match ").bind(query.join(" ")).push(")")
    });
    let substrings = short.into_iter().map(|w| {
        Sql::new("i.id in (select rowid from prompt_fts where typed_text like '%' || ").bind(like_escaped(w)).push(" || '%' escape '\\')")
    });
    Sql::all(phrases.into_iter().chain(substrings))
}

fn like_escaped(word: &str) -> String {
    word.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// A non-empty text value (an empty string is no filter).
fn text(value: Option<&str>) -> Option<&str> {
    value.filter(|v| !v.is_empty())
}

fn equals(column: &str, value: impl ToSqlValue) -> Sql {
    Sql::new(format!("{column} = ")).bind(value.to_value())
}

/// `expr` is one of `values`; an empty list is no filter.
fn listed<T: Serialize>(expr: impl Into<Sql>, values: &[T]) -> DbResult<Option<Sql>> {
    if values.is_empty() { Ok(None) } else { in_list(expr, values).map(Some) }
}

/// The prompt has a row matching `condition` in a per-turn table.
fn of_turn(table: &str, condition: Sql) -> Sql {
    Sql::new(format!("i.id in (select id from {table} where ")).push(condition).push(")")
}

/// The name an enum has in the index (its serde name).
pub(crate) fn name<T: Serialize>(value: &T) -> DbResult<String> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(name) => Ok(name),
        other => Ok(other.to_string()),
    }
}

/// Values a filter compares with (owned into a SQLite value).
trait ToSqlValue {
    fn to_value(self) -> Value;
}

impl ToSqlValue for &str {
    fn to_value(self) -> Value {
        Value::Text(self.to_owned())
    }
}

impl ToSqlValue for String {
    fn to_value(self) -> Value {
        Value::Text(self)
    }
}

impl ToSqlValue for u8 {
    fn to_value(self) -> Value {
        Value::Integer(self.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_are_zero_padded_lower_bounds() {
        assert_eq!(buckets("x", &[0, 20, 80]), "(case when x < 20 then '00000' when x < 80 then '00020' else '00080' end)");
    }

    #[test]
    fn long_words_are_quoted_fts_phrases_and_short_ones_escaped_likes() {
        let sql = words("say \"hi\" a%");
        assert_eq!(
            sql.text(),
            "i.id in (select rowid from prompt_fts where prompt_fts match ?) and \
             i.id in (select rowid from prompt_fts where typed_text like '%' || ? || '%' escape '\\')"
        );
        assert_eq!(sql.params, vec![Value::Text("\"say\" \"\"\"hi\"\"\"".into()), Value::Text("a\\%".into())]);
    }

    #[test]
    fn blank_words_are_no_condition() {
        assert_eq!(words("   ").text(), "1=1");
    }

    #[test]
    fn an_empty_filter_keeps_everything() {
        assert_eq!(conditions(&PromptFilter::default(), "now").unwrap().text(), "1=1");
    }

    #[test]
    fn has_no_follow_up_is_the_negated_follow_up() {
        assert_eq!(has(Has::NoFollowUp), format!("not {FOLLOWED}"));
    }
}
