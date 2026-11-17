//! Names: what you called a session. Durable user data, so neither a reindex nor a clean-up touches
//! them. Also what names a session when you have not (its title and first typed prompts).
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

use std::collections::BTreeMap;

use common::*;
use gitprompt_core::Error;
use gitprompt_core::spec::Origin;
use gitprompt_store::store::{MAX_NAME_CHARS, MAX_NAMED_ID_CHARS, NameKind, clean_name};
use pretty_assertions::assert_eq;

fn ids(sessions: &[&str]) -> Vec<String> {
    sessions.iter().map(ToString::to_string).collect()
}

#[test]
fn a_name_is_set_replaced_read_and_cleared_per_session() {
    let store = store();
    store.set_name(NameKind::Session, "s1", "First").unwrap();
    store.set_name(NameKind::Session, "s2", "Other").unwrap();
    store.set_name(NameKind::Session, "s1", "Renamed").unwrap();

    let names = store.names_of(NameKind::Session, &ids(&["s1", "s2", "s3"])).unwrap();
    assert_eq!(names, BTreeMap::from([("s1".to_string(), "Renamed".to_string()), ("s2".to_string(), "Other".to_string())]));

    assert!(store.clear_name(NameKind::Session, "s1").unwrap());
    assert!(!store.clear_name(NameKind::Session, "s1").unwrap(), "nothing left to forget");
    assert_eq!(store.names_of(NameKind::Session, &ids(&["s1", "s2"])).unwrap().len(), 1);
}

#[test]
fn setting_a_name_returns_it_as_stored() {
    let store = store();
    assert_eq!(store.set_name(NameKind::Session, "s1", "  Fix \n the   parser ").unwrap(), "Fix the parser");
    assert_eq!(store.names_of(NameKind::Session, &ids(&["s1"])).unwrap()["s1"], "Fix the parser");
}

#[test]
fn a_name_must_be_present_short_and_free_of_control_characters() {
    for bad in ["", "   \n ", "a\u{0}b", "bell\u{7}", &"x".repeat(MAX_NAME_CHARS + 1)] {
        assert!(matches!(clean_name(bad), Err(Error::Invalid(_))), "{bad:?}");
    }
    assert_eq!(clean_name(&"é".repeat(MAX_NAME_CHARS)).unwrap().chars().count(), MAX_NAME_CHARS);

    let store = store();
    assert!(matches!(store.set_name(NameKind::Session, "s1", " "), Err(Error::Invalid(_))));
    assert!(store.names_of(NameKind::Session, &ids(&["s1"])).unwrap().is_empty(), "a refused name stores nothing");
}

#[test]
fn what_is_named_must_have_an_id_of_a_sane_length() {
    let store = store();
    for bad in [String::new(), "i".repeat(MAX_NAMED_ID_CHARS + 1)] {
        assert!(matches!(store.set_name(NameKind::Session, &bad, "Name"), Err(Error::Invalid(_))));
    }
}

#[test]
fn a_kind_is_spelled_by_its_name_and_nothing_else() {
    assert_eq!(NameKind::parse("session"), Some(NameKind::Session));
    for other in ["", "Session", "chapter", "sessions"] {
        assert_eq!(NameKind::parse(other), None, "{other:?}");
    }
    assert_eq!(NameKind::Session.as_str(), "session");
}

#[test]
fn names_survive_a_reindex_and_are_kept_by_the_database_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gitprompt.db");
    let store = gitprompt_store::Store::open(&path).unwrap();
    prompt("p1").text("Fix it").session("s1").insert(&store);
    store.set_name(NameKind::Session, "s1", "My session").unwrap();

    // what `gitprompt reindex` does to the index and the event summary
    store.index().drop_all().unwrap();
    store.index().create().unwrap();
    store.rebuild_event_stats().unwrap();
    drop(store);

    let reopened = gitprompt_store::Store::open(&path).unwrap();
    assert_eq!(reopened.names_of(NameKind::Session, &ids(&["s1"])).unwrap()["s1"], "My session");
}

#[test]
fn a_session_opens_with_its_title_and_its_first_typed_prompts_in_order() {
    let store = store();
    prompt("p0").text("injected").session("s1").origin("task_notification").insert(&store);
    prompt("p1").text("/review src/lib.rs fix it").session("s1").insert(&store);
    prompt("p2").text("then this").session("s1").with(|r| r.title = Some("A title".into())).insert(&store);
    prompt("p3").text("and this").session("s1").insert(&store);
    prompt("p4").text("other session").session("s2").insert(&store);

    let openers = store.session_openers(&ids(&["s1", "s2", "s3"]), &[Origin::Typed, Origin::SlashCommand], 2).unwrap();

    let first = &openers["s1"];
    assert_eq!(first.title.as_deref(), Some("A title"));
    assert_eq!(first.typed, ["/review src/lib.rs fix it", "then this"], "the system prompt is skipped, two are kept");
    assert_eq!((openers["s2"].title.clone(), openers["s2"].typed.clone()), (None, vec!["other session".to_string()]));
    assert_eq!(openers.get("s3").map(|o| o.typed.len()), Some(0), "a session without prompts opens with nothing");
}

#[test]
fn a_prompt_without_typed_text_is_not_an_opener() {
    let store = store();
    prompt("p1").session("s1").with(|r| r.typed_text = None).insert(&store);
    prompt("p2").text("words").session("s1").insert(&store);

    let openers = store.session_openers(&ids(&["s1"]), &[Origin::Typed], 3).unwrap();

    assert_eq!(openers["s1"].typed, ["words"]);
}
