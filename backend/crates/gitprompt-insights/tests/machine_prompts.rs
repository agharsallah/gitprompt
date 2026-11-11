//! Prompts a tool sends on its own (another agent's message, a background task's end, a notice that
//! another session went idle) must never count as typed: the Typed filter would list them.
//!
//! The properties are driven by the bundled specs' own rules, not by a list kept here: every marker
//! a spec's origin rule recognises a prompt by (the tags of `text_block`, a `text_prefix`) is tested,
//! so a marker added to a spec is covered with no new test. The cases as captured live are fixtures
//! (testdata/fixtures, checked by conformance.rs).
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

use std::sync::LazyLock;

use proptest::prelude::*;

use common::{prompt, specs};
use gitprompt_core::spec::{Origin, When};
use gitprompt_insights::Specs;
use gitprompt_insights::origin::classify;

static SPECS: LazyLock<Specs> = LazyLock::new(specs);

/// How a rule recognises a prompt by its text alone.
#[derive(Clone, Debug)]
enum Marker {
    /// The prompt opens with a whole `<tag …>…</tag>` block.
    Block(String),
    /// The prompt opens with this text.
    Prefix(String),
}

/// (spec id, marker, origin) for every rule of a bundled spec that looks at the text only.
fn markers() -> Vec<(String, Marker, Origin)> {
    let mut out = Vec::new();
    for spec in SPECS.values() {
        for rule in spec.origin.iter().filter(|rule| text_only(&rule.when)) {
            let found = rule.when.text_block.iter().flatten().map(|tag| Marker::Block(tag.clone()));
            let found = found.chain(rule.when.text_prefix.iter().map(|p| Marker::Prefix(p.clone())));
            out.extend(found.map(|marker| (spec.id.clone(), marker, rule.origin)));
        }
    }
    out
}

fn text_only(when: &When) -> bool {
    when.event.is_none() && when.field.is_none() && when.path.is_none() && when.text_regex.is_none()
}

fn origin(spec_id: &str, text: &str) -> Origin {
    let spec = SPECS.get(spec_id).expect("bundled spec");
    classify(&prompt(text).row(), &[], Some(spec)).origin
}

/// A prompt the marker recognises: the marker around (or before) `body`.
fn marked(marker: &Marker, attributes: &str, body: &str) -> String {
    match marker {
        Marker::Block(tag) => format!("<{tag}{attributes}>{body}</{tag}>"),
        Marker::Prefix(prefix) => format!("{prefix} {body}"),
    }
}

/// Any text, over a few lines, that never opens a tag (so it can't nest or close a block early).
fn body() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 .,:;!?'\"«»—\n-]{0,200}"
}

fn attributes() -> impl Strategy<Value = String> {
    prop_oneof![Just(String::new()), "( [a-z-]{1,10}=\"[a-z0-9:/. -]{0,20}\"){1,3}"]
}

/// Something a person could type first: a word, never a tag, a bracket or a slash command.
fn typed_opening() -> impl Strategy<Value = String> {
    "[A-Za-z][a-zA-Z0-9 ,.?]{0,40}"
}

#[test]
fn claude_code_declares_the_markers_of_what_it_sends_on_its_own() {
    let claude: Vec<String> = markers()
        .into_iter()
        .filter(|(spec, ..)| spec == "claude-code")
        .map(|(_, marker, _)| match marker {
            Marker::Block(tag) => format!("<{tag}>"),
            Marker::Prefix(prefix) => prefix,
        })
        .collect();
    for expected in ["<task-notification>", "<agent-message>", "<cross-session-message>", "[Cross-session idle notice]"] {
        assert!(claude.contains(&expected.to_string()), "{expected} missing from {claude:?}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn every_declared_marker_gives_its_origin_whatever_the_content(
        attributes in attributes(),
        body in body(),
        after in prop_oneof![Just(String::new()), body()],
    ) {
        for (spec, marker, expected) in markers() {
            let text = format!("{}{}", marked(&marker, &attributes, &body), after);
            prop_assert_eq!(origin(&spec, &text), expected, "{} {:?}", spec, text);
        }
    }

    #[test]
    fn a_declared_marker_after_typed_text_leaves_the_prompt_typed(
        opening in typed_opening(),
        attributes in attributes(),
        body in body(),
    ) {
        for (spec, marker, _) in markers() {
            let text = format!("{opening} {}", marked(&marker, &attributes, &body));
            prop_assert_eq!(origin(&spec, &text), Origin::Typed, "{} {:?}", spec, text);
        }
    }
}
