//! `gitprompt reindex`: rebuilds the prompt index and says how many prompts it indexed. Port of
//! cli.reindex (`test_reindex_command`).
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

use common::{home, stdout};
use gitprompt_store::store::NameKind;
use serde_json::json;

fn indexed(out: &str) -> Option<(String, String)> {
    let rest = out.trim_end().strip_prefix("Indexed ")?;
    let (count, rest) = rest.split_once(" prompts in ")?;
    let seconds = rest.strip_suffix(" s.")?;
    Some((count.to_string(), seconds.to_string()))
}

#[tokio::test]
async fn reindex_reports_the_prompts_it_indexed_and_the_time_taken() {
    let home = home();
    let prompt = |pid: &str| json!({"hook_event_name": "UserPromptSubmit", "prompt_id": pid, "session_id": "s", "prompt": "Fix it"});
    home.ingest(&[("claude-code", prompt("p1")), ("claude-code", prompt("p2"))]).await;
    let output = home.cmd(&["reindex"]).assert().success().get_output().clone();
    let (count, seconds) = indexed(&stdout(&output)).unwrap_or_else(|| panic!("{}", stdout(&output)));
    assert_eq!(count, "2");
    assert!(seconds.parse::<f64>().is_ok() && seconds.contains('.'), "{seconds}");
}

#[test]
fn reindex_keeps_the_usage_counts() {
    let home = home();
    let store = gitprompt_store::Store::open(&home.db_path()).unwrap();
    store.increment_usage("reader.tab.events", "2026-11-03", 4).unwrap();
    drop(store);

    home.cmd(&["reindex"]).assert().success();

    let store = gitprompt_store::Store::open(&home.db_path()).unwrap();
    let counts = store.usage_between("2026-11-03", "2026-11-03").unwrap();
    assert_eq!(counts.iter().map(|c| (c.action.as_str(), c.n)).collect::<Vec<_>>(), [("reader.tab.events", 4)]);
}

#[test]
fn reindex_keeps_the_session_names() {
    let home = home();
    let store = gitprompt_store::Store::open(&home.db_path()).unwrap();
    store.set_name(NameKind::Session, "s-1", "My session").unwrap();
    drop(store);

    home.cmd(&["reindex"]).assert().success();

    let store = gitprompt_store::Store::open(&home.db_path()).unwrap();
    let names = store.names_of(NameKind::Session, &["s-1".to_string()]).unwrap();
    assert_eq!(names.get("s-1").map(String::as_str), Some("My session"));
}

#[test]
fn reindex_of_an_empty_home_indexes_nothing() {
    let home = home();
    let output = home.cmd(&["reindex"]).assert().success().get_output().clone();
    assert_eq!(indexed(&stdout(&output)).map(|(count, _)| count), Some("0".to_string()));
}
