//! /api/names/session/{id}: what you call a session. A name shows in the session list (the stats
//! grouped by session) and the story next to the default label (the title its tool reported, else
//! what you first typed), is validated, and needs the dashboard header.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

mod common;

use common::{Api, api, encode, hook_payload, with};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const NAME: &str = "/api/names/session/s1";

/// Session `s1` opened by a slash command and a path, `s2` with a title its tool reported.
async fn data() -> Api {
    let api = api();
    let base = hook_payload(&api.root());
    let prompts = [
        ("p1", "s1", "/review src/lib.rs tidy the parser", json!({})),
        ("p2", "s1", "and then the lexer", json!({})),
        ("p3", "s2", "write the release notes", json!({"session_title": "Release notes"})),
    ];
    for (pid, session, text, extra) in prompts {
        let payload = with(&with(&base, json!({"prompt_id": pid, "session_id": session, "prompt": text})), extra);
        api.ingest("claude-code", payload).await;
    }
    api
}

async fn rename(api: &Api, path: &str, name: &str) -> common::Resp {
    api.put(path).dashboard().json(json!({ "name": name })).send().await
}

/// The `name` and `label` of each session of the list, by session id.
async fn listed(api: &Api) -> Value {
    let stats = api.json("/api/stats?group_by=session").await;
    let groups = stats["groups"].as_array().unwrap();
    Value::Object(
        groups.iter().map(|g| (g["key"].as_str().unwrap().to_string(), json!({"name": g.get("name"), "label": g["label"]}))).collect(),
    )
}

#[tokio::test]
async fn a_session_has_a_default_label_in_the_list_and_the_story_and_no_name() {
    let api = data().await;

    let story = api.json("/api/stories/session/s1").await;
    assert_eq!(story["label"], "tidy the parser");
    assert!(story.get("name").is_none(), "no name is omitted: {story}");
    assert!(story.get("title").is_none(), "the label replaced the title");

    let list = listed(&api).await;
    assert_eq!(list["s1"], json!({"name": null, "label": "tidy the parser"}));
    assert_eq!(list["s2"], json!({"name": null, "label": "Release notes"}), "a title its tool reported is the label");
}

#[tokio::test]
async fn naming_a_session_answers_the_stored_name_and_shows_in_the_list_and_the_story() {
    let api = data().await;

    let put = rename(&api, NAME, "  The   parser\nrewrite ").await;

    assert_eq!((put.status, put.json()), (200, json!({"name": "The parser rewrite"})));
    let story = api.json("/api/stories/session/s1").await;
    assert_eq!((&story["name"], &story["label"]), (&json!("The parser rewrite"), &json!("tidy the parser")));
    let list = listed(&api).await;
    assert_eq!(list["s1"], json!({"name": "The parser rewrite", "label": "tidy the parser"}));
    assert_eq!(list["s2"]["name"], Value::Null, "another session keeps its default");
}

#[tokio::test]
async fn naming_again_replaces_and_deleting_goes_back_to_the_label() {
    let api = data().await;
    rename(&api, NAME, "First").await;
    rename(&api, NAME, "Second").await;
    assert_eq!(api.json("/api/stories/session/s1").await["name"], "Second");

    let gone = api.delete(NAME).dashboard().send().await;
    assert_eq!((gone.status, gone.text()), (204, String::new()));

    let story = api.json("/api/stories/session/s1").await;
    assert!(story.get("name").is_none());
    assert_eq!(listed(&api).await["s1"], json!({"name": null, "label": "tidy the parser"}));
    assert_eq!(api.delete(NAME).dashboard().send().await.status, 204, "forgetting what has no name is fine");
}

#[tokio::test]
async fn a_name_that_does_not_validate_is_refused_and_nothing_is_stored() {
    let api = data().await;
    let long = "x".repeat(81);
    for bad in ["", "   ", "line\u{0}break", long.as_str()] {
        assert_eq!(rename(&api, NAME, bad).await.status, 422, "{bad:?}");
    }
    let no_body = api.put(NAME).dashboard().send().await;
    let wrong_shape = api.put(NAME).dashboard().json(json!({"name": "ok", "extra": 1})).send().await;
    let not_text = api.put(NAME).dashboard().json(json!({"name": 5})).send().await;
    assert_eq!((no_body.status, wrong_shape.status, not_text.status), (422, 422, 422));
    assert!(api.json("/api/stories/session/s1").await.get("name").is_none());
}

#[tokio::test]
async fn the_longest_name_is_accepted() {
    let api = data().await;
    assert_eq!(rename(&api, NAME, &"x".repeat(80)).await.status, 200);
}

#[tokio::test]
async fn writes_need_the_dashboard_header() {
    let api = data().await;
    let put = api.put(NAME).json(json!({"name": "Sneaky"})).send().await;
    assert_eq!(put.status, 403);
    assert_eq!(api.delete(NAME).send().await.status, 403);
    assert!(api.json("/api/stories/session/s1").await.get("name").is_none());
}

#[tokio::test]
async fn an_unknown_session_or_kind_is_not_found() {
    let api = data().await;
    assert_eq!(rename(&api, "/api/names/session/nope", "Name").await.status, 404);
    assert_eq!(rename(&api, "/api/names/chapter/s1", "Name").await.status, 404);
    assert_eq!(rename(&api, "/api/names/whatever/s1", "Name").await.status, 404);
    assert_eq!(api.delete("/api/names/chapter/s1").dashboard().send().await.status, 404);
}

#[tokio::test]
async fn a_session_id_with_odd_characters_is_named_through_its_encoded_path() {
    let api = api();
    let id = "a b/c?d";
    let payload = with(&hook_payload(&api.root()), json!({"session_id": id, "prompt": "odd id"}));
    api.ingest("claude-code", payload).await;

    let path = format!("/api/names/session/{}", encode(id));
    assert_eq!(rename(&api, &path, "Odd").await.status, 200);
    assert_eq!(api.json(&format!("/api/stories/session/{}", encode(id))).await["name"], "Odd");
}
