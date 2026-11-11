//! `Env`: the environment as an injected value.
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

use gitprompt_core::Env;
use pretty_assertions::assert_eq;

#[test]
fn a_set_variable_is_read_back() {
    let env = Env::from_pairs([("GITPROMPT_PORT", "9000")]);
    assert_eq!(env.get("GITPROMPT_PORT"), Some("9000"));
}

#[test]
fn an_unset_variable_is_none() {
    assert_eq!(Env::from_pairs([("A", "1")]).get("B"), None);
    assert_eq!(Env::default().get("PATH"), None);
}

#[test]
fn an_empty_variable_counts_as_unset() {
    assert_eq!(Env::from_pairs([("A", "")]).get("A"), None);
}

#[test]
fn whitespace_is_a_value_not_unset() {
    assert_eq!(Env::from_pairs([("A", " ")]).get("A"), Some(" "));
}

#[test]
fn set_adds_or_replaces_a_variable() {
    let mut env = Env::from_pairs([("A", "1")]);
    env.set("A", "2");
    env.set("B", "3");
    assert_eq!((env.get("A"), env.get("B")), (Some("2"), Some("3")));
}

#[test]
fn process_snapshot_holds_the_process_variables() {
    // Read-only look at the real environment: PATH is set wherever cargo runs.
    let expected = std::env::var("PATH").ok();
    assert!(expected.is_some());
    assert_eq!(Env::process().get("PATH").map(str::to_owned), expected);
}
