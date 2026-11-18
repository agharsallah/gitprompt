//! `gitprompt find` and `gitprompt resume`, run in-process: the library's `run` with an `Io` of
//! buffers and a fixed time, over a home seeded through the server's Service on a fixed clock, so
//! every relative time and every line is exact.
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

use std::sync::Arc;

use clap::Parser;
use jiff::{SignedDuration, Timestamp};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::inprocess::{Call, Ran, run_cli, seed_with};
use common::{Home, home};
use gitprompt_cli::Cli;
use gitprompt_server::{App, AppOptions, FixedClock};

fn t0() -> Timestamp {
    "2026-11-17T12:00:00Z".parse().unwrap()
}

/// Seed `home` as if `ingest` ran at `at`: each payload, then each `(session, name)`.
fn seed(home: &Home, at: Timestamp, payloads: Vec<(&str, Value)>, names: &[(&str, &str)]) {
    seed_with(home, AppOptions { clock: Arc::new(FixedClock(at)), ..home.options() }, payloads, names);
}

fn claude(session: &str, prompt_id: &str, cwd: &str, text: &str) -> (&'static str, Value) {
    let payload = json!({"hook_event_name": "UserPromptSubmit", "session_id": session, "prompt_id": prompt_id, "cwd": cwd, "prompt": text});
    ("claude-code", payload)
}

fn cursor(session: &str, prompt_id: &str, cwd: &str, text: &str) -> (&'static str, Value) {
    let payload = json!({"hook_event_name": "beforeSubmitPrompt", "conversation_id": session, "generation_id": prompt_id, "workspace_roots": [cwd], "prompt": text});
    ("cursor", payload)
}

fn run_in(home: &Home, args: &[&str], tty: bool, no_color: bool) -> Ran {
    run_cli(home, args, Call { tty, no_color, ..Call::at(t0() + SignedDuration::from_mins(90)) })
}

fn run_plain(home: &Home, args: &[&str]) -> Ran {
    run_in(home, args, false, false)
}

/// Two projects, three sessions: s1 (named "release prep") and s2 in Claude Code, s3 in Cursor.
fn seeded() -> Home {
    let home = home();
    seed(&home, t0(), vec![claude("s1", "p1", "/work/api", "Fix the auth bug in login\nthen add a test")], &[("s1", "release prep")]);
    let later =
        vec![claude("s2", "p2", "/work/web", "Add auth to the settings page"), claude("s3x", "p3", "/work/web", "Write the changelog")];
    seed(&home, t0() + SignedDuration::from_mins(1), later, &[("s2", "release prep two")]);
    seed(&home, t0() + SignedDuration::from_mins(2), vec![cursor("c1", "p4", "/work/web", "Tidy the auth middleware")], &[]);
    home
}

#[test]
fn find_lists_numbered_matches_with_when_project_and_first_line() {
    let home = seeded();
    let ran = run_plain(&home, &["find", "auth"]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        "\
1  1 h ago  web  Tidy the auth middleware
2  1 h ago  web  Add auth to the settings page
3  1 h ago  api  Fix the auth bug in login
"
    );
}

#[test]
fn find_without_words_lists_the_latest_prompts() {
    let home = seeded();
    let ran = run_plain(&home, &["find"]);
    assert_eq!(ran.code, 0);
    assert_eq!(ran.out.lines().count(), 4, "{}", ran.out);
    assert!(ran.out.contains("Write the changelog"), "{}", ran.out);
}

#[test]
fn find_narrows_to_a_project_and_limits() {
    let home = seeded();
    let ran = run_plain(&home, &["find", "auth", "--project", "api"]);
    assert_eq!((ran.code, ran.out.as_str()), (0, "1  1 h ago  api  Fix the auth bug in login\n"));
    let ran = run_plain(&home, &["find", "--limit", "2"]);
    assert_eq!(ran.out.lines().count(), 2);
}

#[test]
fn find_with_no_match_says_so_on_stderr_and_exits_1() {
    let home = seeded();
    let ran = run_plain(&home, &["find", "kubernetes"]);
    assert_eq!((ran.code, ran.out.as_str(), ran.err.as_str()), (1, "", "No prompts matching \"kubernetes\".\n"));
    let ran = run_plain(&home, &["find", "auth", "--project", "nowhere"]);
    assert_eq!((ran.code, ran.err.as_str()), (1, "No prompts matching \"auth\" in project \"nowhere\".\n"));
}

#[test]
fn find_json_is_for_scripts_and_is_an_empty_list_with_exit_1_when_nothing_matches() {
    let home = seeded();
    let ran = run_plain(&home, &["find", "auth", "--project", "api", "--json"]);
    let records: Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(ran.code, 0);
    let first = &records[0];
    assert_eq!((first["n"].clone(), first["project"].clone(), first["source"].clone()), (json!(1), json!("api"), json!("claude-code")));
    assert_eq!(first["text"], "Fix the auth bug in login\nthen add a test");

    let none = run_plain(&home, &["find", "kubernetes", "--json"]);
    assert_eq!((none.code, none.out.trim()), (1, "[]"));
}

#[test]
fn print_gives_the_whole_text_of_one_match_and_nothing_else() {
    let home = seeded();
    let ran = run_plain(&home, &["find", "auth", "--print", "3"]);
    assert_eq!((ran.code, ran.out.as_str()), (0, "Fix the auth bug in login\nthen add a test"), "no newline added when piped");
    let on_terminal = run_in(&home, &["find", "auth", "--print", "3"], true, false);
    assert_eq!(on_terminal.out, "Fix the auth bug in login\nthen add a test\n");

    let past_the_end = run_plain(&home, &["find", "auth", "--print", "9"]);
    assert_eq!((past_the_end.code, past_the_end.out.as_str(), past_the_end.err.as_str()), (1, "", "No match 9: 3 prompts matched.\n"));
}

#[test]
fn print_says_one_prompt_matched_not_one_prompts_and_names_the_words_when_none_did() {
    let home = seeded();
    let one = run_plain(&home, &["find", "auth", "--project", "api", "--print", "2"]);
    assert_eq!((one.code, one.err.as_str()), (1, "No match 2: 1 prompt matched.\n"));

    let none = run_plain(&home, &["find", "kubernetes", "--print", "1"]);
    assert_eq!((none.code, none.out.as_str(), none.err.as_str()), (1, "", "No prompts matching \"kubernetes\".\n"));
}

#[test]
fn colour_only_on_a_terminal_and_never_with_no_color() {
    let home = seeded();
    let piped = run_in(&home, &["find", "auth"], false, false);
    let terminal = run_in(&home, &["find", "auth"], true, false);
    let no_color = run_in(&home, &["find", "auth"], true, true);
    assert!(!piped.out.contains('\x1b'), "{}", piped.out);
    assert!(terminal.out.contains("\x1b[36mweb\x1b[0m"), "{}", terminal.out);
    assert_eq!(no_color.out, piped.out);
    let strip = |text: &str| text.replace("\x1b[2m", "").replace("\x1b[36m", "").replace("\x1b[0m", "");
    assert_eq!(strip(&terminal.out), piped.out, "colour never changes the alignment");
}

#[test]
fn resume_prints_cd_and_the_command_and_never_runs_it() {
    let home = seeded();
    let ran = run_plain(&home, &["resume", "s1"]);
    assert_eq!((ran.code, ran.out.as_str(), ran.err.as_str()), (0, "cd /work/api && claude --resume s1\n", ""));
}

#[test]
fn resume_by_name_and_last() {
    let home = seeded();
    let by_name = run_plain(&home, &["resume", "release prep"]);
    assert_eq!((by_name.code, by_name.out.as_str()), (0, "cd /work/api && claude --resume s1\n"), "{}", by_name.err);
    let last = run_plain(&home, &["resume", "--last"]);
    assert_eq!((last.code, last.out.as_str()), (1, ""), "the latest session is the Cursor one, which has no template");

    let home = home_with_latest_claude();
    let last = run_plain(&home, &["resume", "--last"]);
    assert_eq!((last.code, last.out.as_str()), (0, "cd /work/web && claude --resume s9\n"), "{}", last.err);
}

fn home_with_latest_claude() -> Home {
    let home = home();
    seed(&home, t0(), vec![claude("s1", "p1", "/work/api", "older")], &[]);
    seed(&home, t0() + SignedDuration::from_mins(5), vec![claude("s9", "p9", "/work/web", "newer")], &[]);
    home
}

#[test]
fn resume_prints_what_the_dashboard_copies() {
    let home = seeded();
    let service = App::new(home.settings.clone(), home.options()).expect("app").service();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let detail = runtime.block_on(service.prompt(1)).expect("prompt 1");
    let copied = detail.session.resume.expect("a resume action").line();
    assert_eq!(run_plain(&home, &["resume", "s1"]).out, format!("{copied}\n"));
}

#[test]
fn a_name_that_matches_two_sessions_says_which_and_exits_1() {
    let home = seeded();
    let ran = run_plain(&home, &["resume", "release"]);
    assert_eq!((ran.code, ran.out.as_str()), (1, ""));
    assert_eq!(
        ran.err,
        "\
2 sessions match \"release\"; run it again with the id of one:
  s2  1 h ago  release prep two
  s1  1 h ago  release prep
"
    );
}

#[test]
fn a_tool_without_a_template_is_a_clear_message_and_exit_1() {
    let home = seeded();
    let ran = run_plain(&home, &["resume", "c1"]);
    assert_eq!((ran.code, ran.out.as_str()), (1, ""));
    assert_eq!(ran.err, "Cursor has no way to resume a session from the command line, so there is nothing to print.\n");
}

#[test]
fn an_unknown_session_exits_1() {
    let home = seeded();
    let ran = run_plain(&home, &["resume", "nope"]);
    assert_eq!((ran.code, ran.err.as_str()), (1, "No session is called or numbered \"nope\".\n"));
}

#[test]
fn resume_json_carries_the_paste_line_the_folder_and_the_tool() {
    let home = seeded();
    let ran = run_plain(&home, &["resume", "s1", "--json"]);
    let record: Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(
        record,
        json!({"session": "s1", "name": "release prep", "tool": "Claude Code", "command": "claude --resume s1", "cwd": "/work/api", "paste": "cd /work/api && claude --resume s1"})
    );
}

#[test]
fn a_folder_with_a_space_is_quoted() {
    let home = home();
    seed(&home, t0(), vec![claude("s1", "p1", "/work/my app", "hi")], &[]);
    assert_eq!(run_plain(&home, &["resume", "--last"]).out, "cd '/work/my app' && claude --resume s1\n");
}

#[test]
fn without_a_database_both_commands_say_so_and_create_nothing() {
    let home = home();
    for args in [&["find", "x"][..], &["resume", "--last"][..]] {
        let ran = run_plain(&home, args);
        assert_eq!(ran.code, 1);
        assert!(ran.err.starts_with("No database yet at "), "{}", ran.err);
    }
    assert!(!home.settings.home.exists());
}

#[test]
fn resume_wants_a_session_or_last_but_not_both() {
    assert!(Cli::try_parse_from(["gitprompt", "resume"]).is_err());
    assert!(Cli::try_parse_from(["gitprompt", "resume", "s1", "--last"]).is_err());
    assert!(Cli::try_parse_from(["gitprompt", "find", "x", "--limit", "0"]).is_err());
    assert!(Cli::try_parse_from(["gitprompt", "find", "x", "--print", "1", "--json"]).is_err());
}
