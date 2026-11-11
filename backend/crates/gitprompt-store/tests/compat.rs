//! The file on disk: opening a database the Python daemon wrote (no migration), file and directory
//! modes, WAL, reopening, and online backup.
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

use std::path::Path;

use common::*;
use gitprompt_store::index::{IndexRow, UsageTotals};
use gitprompt_store::rows::{Annotation, EventRow, PromptRow};
use gitprompt_store::{PromptFilter, Store};
use pretty_assertions::assert_eq;
use rusqlite::Connection;
use serde_json::json;

/// storage/store.py SCHEMA, with `{session}` and `{view}` expanded as the Python daemon runs it.
const PY_STORE_SCHEMA: &str = r"
create table if not exists prompt (
  id          integer primary key,
  source      text not null,
  received_at text not null,
  prompt      text,
  fields      text not null,
  context     text not null,
  raw         text not null
);
create unique index if not exists prompt_dedup on prompt (source, json_extract(fields, '$.prompt_id'));
create index if not exists prompt_recent on prompt (received_at desc);
create index if not exists prompt_turn on prompt (json_extract(fields, '$.prompt_id'));

create table if not exists event (
  id          integer primary key,
  source      text not null,
  received_at text not null,
  event       text,
  session_id  text,
  prompt_id   text,
  fields      text not null,
  raw         text not null
);
create index if not exists event_prompt on event (prompt_id);
create index if not exists event_session on event (session_id);

create table if not exists annotation (id integer primary key, data text not null);

create index if not exists prompt_session on prompt (source, coalesce(json_extract(fields, '$.session_id'), ''));
drop view if exists prompt_view;
create view prompt_view as
select p.*, coalesce(a.data, '{}') annotation,
       row_number() over w turn,
       lag(p.received_at) over w prev_at,
       lead(json_extract(p.context, '$.git_head')) over w next_head
from prompt p left join annotation a on a.id = p.id
where 1=1
window w as (partition by p.source, coalesce(json_extract(p.fields, '$.session_id'), '') order by p.id);
";

/// storage/index.py SCHEMA.
const PY_INDEX_SCHEMA: &str = r"
create table if not exists index_meta (key text primary key, value text not null);
create table if not exists prompt_index (
  id integer primary key, prompt_id text, source text not null, session text, repo text, repo_name text,
  branch text, title text, received_at text not null, day text not null, hour integer not null,
  weekday integer not null, origin text not null, chars integer not null, text_hash text, slash_command text,
  code_blocks integer not null, urls integer not null, mentions integer not null, secrets integer not null,
  api_requests integer not null, cost_usd real not null, input_tokens integer not null, output_tokens integer not null,
  cache_read_tokens integer not null, cache_creation_tokens integer not null, api_time_s real not null,
  api_errors integer not null, response_s real, ended_by text, tool_calls integer not null,
  tool_failures integer not null, tool_time_s real not null, subagent_tool_calls integer not null,
  subagents integer not null, files_written integer not null, git_commits integer not null,
  compactions integer not null, denials integer not null, rejections integer not null, model text,
  permission_mode text, tool_version text, reasoning_tokens integer not null, answered integer not null,
  surface text
);
create index if not exists prompt_index_time on prompt_index (received_at);
create index if not exists prompt_index_origin on prompt_index (origin, received_at);
create index if not exists prompt_index_session on prompt_index (session, id);
create index if not exists prompt_index_repo on prompt_index (repo);
create table if not exists prompt_index_model (
  id integer not null, model text, api_requests integer not null, cost_usd real not null,
  input_tokens integer not null, output_tokens integer not null,
  cache_read_tokens integer not null, cache_creation_tokens integer not null
);
create index if not exists prompt_index_model_id on prompt_index_model (id);
create table if not exists prompt_index_tool (id integer not null, tool text, kind text not null, calls integer not null);
create index if not exists prompt_index_tool_id on prompt_index_tool (id);
create table if not exists prompt_index_work (
  id integer not null, work text not null, query_source text, api_requests integer not null, cost_usd real not null,
  input_tokens integer not null, output_tokens integer not null,
  cache_read_tokens integer not null, cache_creation_tokens integer not null
);
create index if not exists prompt_index_work_id on prompt_index_work (id);
create table if not exists session_metric (
  event_id integer primary key, session text, received_at text not null, name text not null, value real not null
);
create index if not exists session_metric_session on session_metric (session, received_at);
create table if not exists session_command (
  event_id integer primary key, session text, received_at text not null, command text not null, source text
);
create index if not exists session_command_session on session_command (session, received_at);
create table if not exists index_dirty (prompt_id text primary key);
create virtual table if not exists prompt_fts using fts5(typed_text, tokenize = 'trigram');
";

/// Rows as the Python daemon writes them: `json.dumps` (spaces after separators, non-ASCII
/// escaped), `received_at` from `datetime.isoformat()`.
const PY_ROWS: &str = r#"
insert into prompt values (1, 'claude-code', '2026-11-03T10:00:00.123456+00:00', 'Write a factorial function',
  '{"event": "UserPromptSubmit", "session_id": "abc123", "prompt_id": "p1", "prompt": "Write a factorial function", "cwd": "/work/café"}',
  '{"git_head": "aaa", "tool_version": "2.1.316"}',
  '{"hook_event_name": "UserPromptSubmit", "session_id": "abc123", "prompt_id": "p1", "prompt": "Write a factorial function", "n": 1.5}');
insert into prompt values (2, 'claude-code', '2026-11-03T10:05:00.000001+00:00', null,
  '{"event": "UserPromptSubmit", "session_id": "abc123", "prompt_id": "p2"}', '{"git_head": "bbb"}', '{"text": [1, 2]}');
insert into event values (1, 'claude-code', '2026-11-03T10:00:30.5+00:00', 'Stop', 'abc123', 'p1',
  '{"event": "Stop", "session_id": "abc123", "prompt_id": "p1"}', '{"hook_event_name": "Stop", "last_assistant_message": "Done"}');
insert into annotation values (1, '{"starred": true, "tags": ["release"]}');
insert into index_meta values ('rules_version', 'py-rules-1');
insert into index_meta values ('telemetry_event_id', '12');
insert into index_dirty values ('p2');
insert into prompt_index values (1, 'p1', 'claude-code', 'abc123', '/work/app', 'me/app', 'main', null,
  '2026-11-03T10:00:00.123456+00:00', '2026-11-03', 11, 2, 'typed', 26, 'h1', null, 0, 0, 0, 0,
  1, 0.5, 10, 20, 30, 40, 1.5, 0, 30.5, 'Stop', 2, 0, 0.25, 0, 0, 1, 0, 0, 0, 0,
  'opus', 'default', '2.1.316', 5, 1, 'cli');
insert into prompt_index_model values (1, 'opus', 1, 0.5, 10, 20, 30, 40);
insert into prompt_index_tool values (1, 'Edit', 'write', 2);
insert into prompt_fts (rowid, typed_text) values (1, 'Write a factorial function');
"#;

fn python_database(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "journal_mode", "wal").unwrap();
    conn.execute_batch(PY_STORE_SCHEMA).unwrap();
    conn.execute_batch(PY_INDEX_SCHEMA).unwrap();
    conn.execute_batch(PY_ROWS).unwrap();
}

fn row_counts(path: &Path) -> Vec<i64> {
    let conn = Connection::open(path).unwrap();
    ["prompt", "event", "annotation", "prompt_index", "prompt_index_model", "prompt_index_tool", "prompt_fts", "index_meta", "index_dirty"]
        .iter()
        .map(|table| conn.query_row(&format!("select count(*) from {table}"), [], |r| r.get(0)).unwrap())
        .collect()
}

fn opened_python_database() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    python_database(&path);
    let store = Store::open(&path).unwrap();
    (dir, store)
}

#[test]
fn prompts_written_by_the_python_daemon_read_back_unchanged() {
    let (_dir, store) = opened_python_database();

    let first = PromptRow {
        id: 1,
        source: "claude-code".into(),
        received_at: "2026-11-03T10:00:00.123456+00:00".into(),
        prompt: Some("Write a factorial function".into()),
        fields: obj(json!({
            "event": "UserPromptSubmit", "session_id": "abc123", "prompt_id": "p1",
            "prompt": "Write a factorial function", "cwd": "/work/café",
        })),
        context: obj(json!({"git_head": "aaa", "tool_version": "2.1.316"})),
        raw: json!({
            "hook_event_name": "UserPromptSubmit", "session_id": "abc123", "prompt_id": "p1",
            "prompt": "Write a factorial function", "n": 1.5,
        }),
    };
    let second = PromptRow {
        id: 2,
        source: "claude-code".into(),
        received_at: "2026-11-03T10:05:00.000001+00:00".into(),
        prompt: None,
        fields: obj(json!({"event": "UserPromptSubmit", "session_id": "abc123", "prompt_id": "p2"})),
        context: obj(json!({"git_head": "bbb"})),
        raw: json!({"text": [1, 2]}),
    };
    assert_eq!(store.prompts(&[1, 2]).unwrap(), vec![first, second]);
}

#[test]
fn python_sessions_events_and_annotations_read_back_unchanged() {
    let (_dir, store) = opened_python_database();

    let first = store.get(1).unwrap().unwrap();
    let second = store.get(2).unwrap().unwrap();

    assert_eq!((first.turn, first.next_head.as_deref()), (1, Some("bbb")));
    assert_eq!((second.turn, second.prev_at.as_deref()), (2, Some("2026-11-03T10:00:00.123456+00:00")));
    assert_eq!(first.annotation, Annotation { starred: true, tags: vec!["release".into()], ..Default::default() });
    let stop = EventRow {
        id: 1,
        source: "claude-code".into(),
        received_at: "2026-11-03T10:00:30.5+00:00".into(),
        event: Some("Stop".into()),
        session_id: Some("abc123".into()),
        prompt_id: Some("p1".into()),
        fields: obj(json!({"event": "Stop", "session_id": "abc123", "prompt_id": "p1"})),
        raw: json!({"hook_event_name": "Stop", "last_assistant_message": "Done"}),
    };
    assert_eq!(store.events_for_prompts(&["p1".into()]).unwrap(), vec![stop]);
}

#[test]
fn the_python_prompt_index_is_kept_and_usable() {
    let (_dir, store) = opened_python_database();

    let expected = IndexRow {
        id: 1,
        prompt_id: Some("p1".into()),
        source: "claude-code".into(),
        session: Some("abc123".into()),
        repo: Some("/work/app".into()),
        repo_name: Some("me/app".into()),
        branch: Some("main".into()),
        title: None,
        received_at: "2026-11-03T10:00:00.123456+00:00".into(),
        day: "2026-11-03".into(),
        hour: 11,
        weekday: 2,
        origin: "typed".into(),
        chars: 26,
        text_hash: Some("h1".into()),
        usage: UsageTotals {
            api_requests: 1,
            cost_usd: 0.5,
            input_tokens: 10,
            output_tokens: 20,
            cache_read_tokens: 30,
            cache_creation_tokens: 40,
        },
        api_time_s: 1.5,
        response_s: Some(30.5),
        ended_by: Some("Stop".into()),
        tool_calls: 2,
        tool_time_s: 0.25,
        files_written: 1,
        model: Some("opus".into()),
        permission_mode: Some("default".into()),
        tool_version: Some("2.1.316".into()),
        reasoning_tokens: 5,
        answered: true,
        surface: Some("cli".into()),
        ..IndexRow::default()
    };
    let row = store.index().row(1).unwrap().map(|r| IndexRow { typed_text: None, ..r });
    assert_eq!(row, Some(expected));
    assert_eq!(store.index().version().unwrap().as_deref(), Some("py-rules-1"));
    assert_eq!(store.index().telemetry_after().unwrap(), 12);
    assert_eq!(store.index().take_marked().unwrap(), vec!["p2".to_string()]);
    assert_eq!(found(&store, &PromptFilter { q: Some("FACTORIAL".into()), ..Default::default() }), vec![1]);
    assert_eq!(found(&store, &PromptFilter { model: vec!["opus".into()], ..Default::default() }), vec![1]);
}

#[test]
fn opening_a_python_database_keeps_every_row() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    python_database(&path);
    let before = row_counts(&path);

    Store::open(&path).unwrap(); // opened, then closed at the end of the statement

    assert_eq!(row_counts(&path), before);
}

#[test]
fn a_python_database_still_deduplicates_prompt_ids() {
    let (_dir, store) = opened_python_database();
    let again = obj(json!({"prompt_id": "p1", "session_id": "abc123"}));

    assert_eq!(store.add_prompt("claude-code", &again, &obj(json!({})), &json!({})).unwrap(), None);
    assert!(store.add_prompt("codex", &again, &obj(json!({})), &json!({})).unwrap().is_some());
}

#[test]
fn a_database_written_by_the_rust_store_reopens_with_its_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    let (id, stop) = {
        let store = Store::open(&path).unwrap();
        (prompt("p1").text("Fix it").insert(&store), event("Stop").prompt_id("p1").insert(&store))
    };

    let store = Store::open(&path).unwrap();

    assert_eq!(store.get(id).unwrap().unwrap().row.prompt.as_deref(), Some("Fix it"));
    assert_eq!(store.events_for_prompts(&["p1".into()]).unwrap(), vec![stop]);
    assert!(store.index().row(id).unwrap().is_some());
}

#[test]
fn the_database_is_in_wal_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    Store::open(&path).unwrap(); // opened, then closed at the end of the statement

    let mode: String = Connection::open(&path).unwrap().query_row("pragma journal_mode", [], |r| r.get(0)).unwrap();

    assert_eq!(mode, "wal");
}

#[cfg(unix)]
mod modes {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use gitprompt_store::Store;

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn opening_creates_a_private_data_directory_and_database_file() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("gitprompt");
        let path = home.join("gitprompt.db");

        Store::open(&path).unwrap(); // opened, then closed at the end of the statement

        assert_eq!((mode(&home), mode(&path)), (0o700, 0o600));
    }

    #[test]
    fn opening_an_existing_database_makes_it_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gitprompt.db");
        Store::open(&path).unwrap(); // opened, then closed at the end of the statement
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        Store::open(&path).unwrap(); // opened, then closed at the end of the statement

        assert_eq!(mode(&path), 0o600);
    }
}

#[test]
fn a_backup_is_a_readable_copy_and_the_store_keeps_working() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("gitprompt.db")).unwrap();
    let id = prompt("p1").text("Fix the login bug").insert(&store);
    let stop = event("Stop").prompt_id("p1").insert(&store);
    store.annotate(id, &Annotation { starred: true, ..Default::default() }).unwrap();
    let backup = dir.path().join("backup.db");

    store.backup_to(&backup).unwrap();
    let later = prompt("p2").insert(&store);

    let copy = Store::open(&backup).unwrap();
    assert_eq!(copy.prompts(&[id]).unwrap(), store.prompts(&[id]).unwrap());
    assert_eq!(copy.events_for_prompts(&["p1".into()]).unwrap(), vec![stop]);
    assert_eq!(copy.annotation(id).unwrap(), store.annotation(id).unwrap());
    assert_eq!(found(&copy, &PromptFilter { q: Some("login".into()), ..Default::default() }), vec![id]);
    assert_eq!(copy.prompt_ids().unwrap(), vec![id]);
    assert_eq!(store.prompt_ids().unwrap(), vec![id, later]);
}
