//! `gitprompt reindex`: rebuilds the prompt index and says how many prompts it indexed. Port of
//! cli.reindex (test_reindex_command).

mod common;

use common::{home, stdout};
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
fn reindex_of_an_empty_home_indexes_nothing() {
    let home = home();
    let output = home.cmd(&["reindex"]).assert().success().get_output().clone();
    assert_eq!(indexed(&stdout(&output)).map(|(count, _)| count), Some("0".to_string()));
}
