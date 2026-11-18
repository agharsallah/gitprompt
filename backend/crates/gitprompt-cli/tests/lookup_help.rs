//! `--help` of `find`, `resume` and `why` shows lines to copy, and says what the exit codes mean.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

mod common;

use common::home;
use predicates::str::contains;

#[test]
fn find_help_has_three_example_lines_and_the_exit_codes() {
    home()
        .cmd(&["find", "--help"])
        .assert()
        .success()
        .stdout(contains("gitprompt find auth --print 2 | pbcopy"))
        .stdout(contains("gitprompt find --project api --json"))
        .stdout(contains("Exits 0 when something matched, 1 when nothing did"));
}

#[test]
fn resume_help_has_three_example_lines_and_the_exit_codes() {
    home()
        .cmd(&["resume", "--help"])
        .assert()
        .success()
        .stdout(contains("gitprompt resume --last | pbcopy"))
        .stdout(contains("gitprompt resume \"release prep\""))
        .stdout(contains("Exits 0 when a command was printed, 1 when there is none"));
}

#[test]
fn why_help_has_example_lines_and_the_exit_codes() {
    home()
        .cmd(&["why", "--help"])
        .assert()
        .success()
        .stdout(contains("gitprompt why --branch timing-ledger | pbcopy"))
        .stdout(contains("gitprompt why --branch main --project api"))
        .stdout(contains("Exits 0 when the block was printed, 1 when no prompt you typed was recorded"));
}

#[test]
fn the_binary_exits_1_with_a_note_before_there_is_a_database() {
    let home = home();
    home.cmd(&["find", "x"]).assert().code(1).stderr(contains("No database yet"));
    home.cmd(&["resume", "--last"]).assert().code(1).stderr(contains("No database yet"));
    home.cmd(&["why", "--branch", "main"]).assert().code(1).stderr(contains("No database yet"));
}
