//! Statistics per session (telemetry) and per connector (latest), not over prompt buckets.

use rusqlite::Connection;

use super::{CommandUse, Latest, Telemetry};
use crate::decode::{self, text_at};
use crate::error::DbResult;
use crate::filter::PromptFilter;
use crate::sql::Sql;
use crate::sql::filter::conditions;

/// Telemetry metric totals and slash commands over the sessions of the prompts the filter keeps,
/// and within `since`/`until` when given. Telemetry arrives per session, not per prompt.
pub(super) fn telemetry(conn: &Connection, filter: &PromptFilter, running_since: &str) -> DbResult<Telemetry> {
    let sessions = Sql::new("select distinct i.session from prompt_index i where ")
        .push(conditions(filter, running_since)?)
        .push(" and i.session is not null");
    let mut scope = Sql::new("t.session in (").push(sessions.clone()).push(")");
    if let Some(since) = filter.since.as_deref().filter(|s| !s.is_empty()) {
        scope = scope.push(" and t.received_at >= ").bind(since.to_owned());
    }
    if let Some(until) = filter.until.as_deref().filter(|s| !s.is_empty()) {
        scope = scope.push(" and t.received_at < ").bind(until.to_owned());
    }

    let count = Sql::new("select count(*) from (").push(sessions).push(")");
    let sessions = conn.prepare_cached(count.text())?.query_row(count.params(), |row| row.get(0))?;
    let metrics =
        Sql::new("select name, total(value) value from session_metric t where ").push(scope.clone()).push(" group by 1 order by 1");
    let metrics =
        decode::all(conn, metrics.text(), metrics.params(), |row| Ok((row.get::<_, String>("name")?, round4(row.get("value")?))))?;
    let commands = Sql::new("select command, source, count(*) uses, count(distinct session) sessions from session_command t where ")
        .push(scope)
        .push(" group by 1, 2 order by uses desc, command");
    let commands = decode::all(conn, commands.text(), commands.params(), |row| {
        Ok(CommandUse {
            command: row.get("command")?,
            source: text_at(row, "source")?,
            uses: row.get("uses")?,
            sessions: row.get("sessions")?,
        })
    })?;
    Ok(Telemetry { sessions, metrics: metrics.into_iter().collect(), commands })
}

/// For each connector and surface: its newest prompt and its prompt count, newest first.
pub(super) fn latest(conn: &Connection) -> DbResult<Vec<Latest>> {
    let select = "select i.id, i.source, i.surface, i.received_at, i.session, i.repo, i.repo_name, i.branch, i.model, i.origin, i.title, n.prompts \
         from prompt_index i \
         join (select max(id) id, count(*) prompts from prompt_index group by source, surface) n on n.id = i.id \
         order by i.id desc";
    decode::all(conn, select, [], |row| {
        Ok(Latest {
            id: row.get("id")?,
            source: row.get("source")?,
            surface: text_at(row, "surface")?,
            received_at: row.get("received_at")?,
            session: text_at(row, "session")?,
            repo: text_at(row, "repo")?,
            repo_name: text_at(row, "repo_name")?,
            branch: text_at(row, "branch")?,
            model: text_at(row, "model")?,
            origin: row.get("origin")?,
            title: text_at(row, "title")?,
            prompts: row.get("prompts")?,
        })
    })
}

/// Rounded to 4 decimals, ties to even (as Python's `round`).
fn round4(value: f64) -> f64 {
    gitprompt_core::numbers::round(value, 4)
}
