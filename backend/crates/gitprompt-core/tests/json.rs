//! Payload helpers (port of `tests/core/test_payload.py`, plus the edges payload.py handles).
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

use common::arb_json;
use gitprompt_core::json::{CLIP_LIMIT, Mapping, clip, compact, extract, first, flatten, is_no_value, map_strings, pluck};
use gitprompt_core::{Json, JsonMap};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use serde_json::json;

fn obj(value: Json) -> JsonMap {
    match value {
        Json::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

fn paths(candidates: &[&str]) -> Vec<String> {
    candidates.iter().map(std::string::ToString::to_string).collect()
}

// --- pluck ----------------------------------------------------------------------------------

#[test]
fn pluck_follows_dots_and_indexes() {
    let data = json!({"a": {"b": [{"c": 1}]}});
    assert_eq!(pluck(&data, "a.b[0].c"), Some(&json!(1)));
    assert_eq!(pluck(&data, "a.b[5].c"), None);
    assert_eq!(pluck(&data, "a.x"), None);
}

#[test]
fn pluck_returns_whole_subtrees() {
    let data = json!({"message": {"content": "hi", "role": "user"}});
    assert_eq!(pluck(&data, "message"), Some(&json!({"content": "hi", "role": "user"})));
    assert_eq!(pluck(&data, "message.content"), Some(&json!("hi")));
}

#[test]
fn pluck_matches_a_key_containing_dots_before_walking_the_path() {
    let data = json!({"prompt.id": "flat", "prompt": {"id": "nested"}});
    assert_eq!(pluck(&data, "prompt.id"), Some(&json!("flat")));
}

#[test]
fn pluck_walks_the_path_when_no_key_has_the_dotted_name() {
    let data = json!({"prompt": {"id": "nested"}});
    assert_eq!(pluck(&data, "prompt.id"), Some(&json!("nested")));
}

#[test]
fn pluck_reads_the_first_workspace_root() {
    let data = json!({"workspace_roots": ["/a", "/b"]});
    assert_eq!(pluck(&data, "workspace_roots[0]"), Some(&json!("/a")));
    assert_eq!(pluck(&data, "workspace_roots[1]"), Some(&json!("/b")));
}

#[test]
fn pluck_accepts_a_dotted_index_like_a_bracketed_one() {
    let data = json!({"a": ["x", "y"]});
    assert_eq!(pluck(&data, "a.1"), Some(&json!("y")));
}

#[test]
fn pluck_indexes_a_top_level_list() {
    assert_eq!(pluck(&json!([10, 20]), "[1]"), Some(&json!(20)));
}

#[test]
fn pluck_reads_a_digit_key_of_an_object() {
    assert_eq!(pluck(&json!({"a": {"0": "zero"}}), "a[0]"), Some(&json!("zero")));
}

#[test]
fn pluck_gives_none_for_an_index_just_past_the_end() {
    assert_eq!(pluck(&json!({"a": [1, 2]}), "a[2]"), None);
}

#[test]
fn pluck_gives_none_for_a_negative_or_non_numeric_list_index() {
    let data = json!({"a": [1, 2]});
    assert_eq!(pluck(&data, "a[-1]"), None);
    assert_eq!(pluck(&data, "a[x]"), None);
    assert_eq!(pluck(&data, "a.first"), None);
}

#[test]
fn pluck_gives_none_for_an_index_too_big_for_any_integer() {
    assert_eq!(pluck(&json!({"a": [1]}), "a[99999999999999999999999]"), None);
}

#[test]
fn pluck_gives_none_when_a_step_lands_on_a_scalar() {
    assert_eq!(pluck(&json!({"a": 1}), "a.b"), None);
    assert_eq!(pluck(&json!({"a": "text"}), "a[0]"), None);
    assert_eq!(pluck(&json!("text"), "a"), None);
}

// --- is_no_value / first / extract ----------------------------------------------------------

#[test]
fn null_empty_string_and_empty_list_are_no_value() {
    assert!(is_no_value(&json!(null)));
    assert!(is_no_value(&json!("")));
    assert!(is_no_value(&json!([])));
}

#[test]
fn empty_object_zero_false_and_whitespace_are_values() {
    let values = [json!({}), json!(0), json!(false), json!(" "), json!([null])];
    assert_eq!(values.iter().map(is_no_value).collect::<Vec<_>>(), [false; 5]);
}

#[test]
fn first_skips_candidates_without_a_value() {
    let data = json!({"a": null, "b": "", "c": [], "d": "found", "e": "later"});
    assert_eq!(first(&data, &paths(&["missing", "a", "b", "c", "d", "e"])), Some(&json!("found")));
}

#[test]
fn first_keeps_false_zero_and_empty_object() {
    assert_eq!(first(&json!({"a": false}), &paths(&["a", "b"])), Some(&json!(false)));
    assert_eq!(first(&json!({"a": 0}), &paths(&["a"])), Some(&json!(0)));
    assert_eq!(first(&json!({"a": {}}), &paths(&["a"])), Some(&json!({})));
}

#[test]
fn first_is_none_when_no_candidate_has_a_value() {
    assert_eq!(first(&json!({"a": ""}), &paths(&["a", "b"])), None);
    assert_eq!(first(&json!({"a": 1}), &[]), None);
}

#[test]
fn extract_takes_first_non_empty_candidate() {
    let mapping: Mapping =
        [("session_id".to_string(), paths(&["session_id", "sessionId"])), ("model".to_string(), paths(&["model"]))].into_iter().collect();
    assert_eq!(extract(&json!({"session_id": "", "sessionId": "s1"}), &mapping), obj(json!({"session_id": "s1"})));
}

#[test]
fn extract_orders_fields_as_the_mapping_does() {
    let mapping: Mapping =
        [("z".to_string(), paths(&["z"])), ("a".to_string(), paths(&["a"])), ("m".to_string(), paths(&["m"]))].into_iter().collect();
    let found = extract(&json!({"a": 1, "m": false, "z": {"k": 2}}), &mapping);
    assert_eq!(found.keys().collect::<Vec<_>>(), ["z", "a", "m"]);
    assert_eq!(Json::Object(found), json!({"z": {"k": 2}, "a": 1, "m": false}));
}

#[test]
fn extract_of_a_non_object_payload_is_empty() {
    let mapping: Mapping = [("a".to_string(), paths(&["a"]))].into_iter().collect();
    assert_eq!(extract(&json!(["a"]), &mapping), JsonMap::new());
}

// --- map_strings / clip ---------------------------------------------------------------------

#[test]
fn map_strings_changes_strings_only_and_never_keys() {
    let value = json!({"key": "v", "list": ["a", 1, null, true, {"inner": "b"}], "n": 2.5});
    let upper = map_strings(&value, &|s| s.to_uppercase());
    assert_eq!(upper, json!({"key": "V", "list": ["A", 1, null, true, {"inner": "B"}], "n": 2.5}));
}

#[test]
fn map_strings_maps_a_bare_string() {
    assert_eq!(map_strings(&json!("ab"), &|s| format!("<{s}>")), json!("<ab>"));
}

#[test]
fn clip_keeps_short_values() {
    let value = json!({"a": "x".repeat(5), "b": ["y".repeat(20)]});
    let expected = json!({"a": "xxxxx", "b": ["y".repeat(10) + "… [10 more characters not stored]"]});
    assert_eq!(clip(&value, 10), expected);
}

#[test]
fn clip_keeps_a_string_of_exactly_the_limit() {
    assert_eq!(clip(&json!("x".repeat(10)), 10), json!("x".repeat(10)));
}

#[test]
fn clip_shortens_a_string_one_past_the_limit() {
    assert_eq!(clip(&json!("x".repeat(11)), 10), json!("x".repeat(10) + "… [1 more characters not stored]"));
}

#[test]
fn clip_counts_characters_not_bytes() {
    // 10 two-byte characters fit a limit of 10; 11 don't, and the cut never splits a character.
    assert_eq!(clip(&json!("é".repeat(10)), 10), json!("é".repeat(10)));
    assert_eq!(clip(&json!("日本語".repeat(4)), 10), json!("日本語日本語日本語日… [2 more characters not stored]"));
}

#[test]
fn clip_leaves_numbers_bools_and_nulls_alone() {
    let value = json!([123_456_789_012_i64, true, null]);
    assert_eq!(clip(&value, 3), value);
}

#[test]
fn clip_limit_default_is_ten_thousand_characters() {
    assert_eq!(CLIP_LIMIT, 10_000);
    let long = json!("a".repeat(CLIP_LIMIT + 5));
    assert_eq!(clip(&long, CLIP_LIMIT), json!("a".repeat(CLIP_LIMIT) + "… [5 more characters not stored]"));
}

// --- flatten --------------------------------------------------------------------------------

#[test]
fn flatten_joins_keys_with_dots_and_marks_list_items() {
    let value = json!({"a": {"b": 1}, "c": [1, {"d": "x"}]});
    let expected = vec![("a.b".to_string(), json!(1)), ("c[]".to_string(), json!(1)), ("c[].d".to_string(), json!("x"))];
    assert_eq!(flatten(&value), expected);
}

#[test]
fn flatten_keeps_empty_containers_as_leaves() {
    let expected = vec![("a".to_string(), json!({})), ("b".to_string(), json!([]))];
    assert_eq!(flatten(&json!({"a": {}, "b": []})), expected);
}

#[test]
fn flatten_of_a_scalar_or_empty_root_is_one_unnamed_leaf() {
    assert_eq!(flatten(&json!(3)), vec![(String::new(), json!(3))]);
    assert_eq!(flatten(&json!({})), vec![(String::new(), json!({}))]);
}

#[test]
fn flatten_of_nested_lists_repeats_the_marker() {
    assert_eq!(flatten(&json!([[1]])), vec![("[][]".to_string(), json!(1))]);
}

// --- compact --------------------------------------------------------------------------------

#[test]
fn compact_drops_empty_false_and_zero_values() {
    let data = obj(json!({
        "null": null, "empty": "", "list": [], "map": {}, "no": false, "zero": 0, "zero_float": 0.0,
        "text": "0", "yes": true, "one": 1, "neg": -1, "half": 0.5, "nested": {"a": 0}, "items": [0]
    }));
    let expected = obj(json!({"text": "0", "yes": true, "one": 1, "neg": -1, "half": 0.5, "nested": {"a": 0}, "items": [0]}));
    assert_eq!(compact(&data), expected);
}

#[test]
fn compact_keeps_key_order() {
    let kept = compact(&obj(json!({"z": 1, "a": 0, "m": 2})));
    assert_eq!(kept.keys().collect::<Vec<_>>(), ["z", "m"]);
}

// --- properties -----------------------------------------------------------------------------

fn is_empty_or_zero(value: &Json) -> bool {
    match value {
        Json::Null => true,
        Json::Bool(b) => !b,
        Json::Number(n) => n.as_f64() == Some(0.0),
        Json::String(s) => s.is_empty(),
        Json::Array(a) => a.is_empty(),
        Json::Object(o) => o.is_empty(),
    }
}

proptest! {
    #[test]
    fn map_strings_with_identity_returns_the_same_value(value in arb_json()) {
        prop_assert_eq!(map_strings(&value, &|s| s.to_string()), value);
    }

    #[test]
    fn map_strings_keeps_every_path(value in arb_json()) {
        let marked = map_strings(&value, &|s| format!("{s}!"));
        let before: Vec<String> = flatten(&value).into_iter().map(|(p, _)| p).collect();
        let after: Vec<String> = flatten(&marked).into_iter().map(|(p, _)| p).collect();
        prop_assert_eq!(before, after);
    }

    #[test]
    fn clip_never_changes_a_string_within_the_limit(text in "\\PC{0,40}", extra in 0usize..10) {
        let limit = text.chars().count() + extra;
        prop_assert_eq!(clip(&json!(text), limit), json!(text));
    }

    #[test]
    fn clip_keeps_the_first_limit_characters_and_counts_the_rest(text in "\\PC{1,60}", limit in 0usize..40) {
        let chars = text.chars().count();
        prop_assume!(chars > limit);
        let head: String = text.chars().take(limit).collect();
        let expected = format!("{head}… [{} more characters not stored]", chars - limit);
        prop_assert_eq!(clip(&json!(text), limit), json!(expected));
    }

    #[test]
    fn flatten_leaves_are_never_non_empty_containers(value in arb_json()) {
        let containers: Vec<Json> = flatten(&value)
            .into_iter()
            .map(|(_, leaf)| leaf)
            .filter(|leaf| matches!(leaf, Json::Array(a) if !a.is_empty()) || matches!(leaf, Json::Object(o) if !o.is_empty()))
            .collect();
        prop_assert_eq!(containers, Vec::<Json>::new());
    }

    #[test]
    fn compact_keeps_exactly_the_non_empty_entries_and_is_idempotent(value in prop::collection::btree_map("[a-z]{1,4}", arb_json(), 0..8)) {
        let data: JsonMap = value.into_iter().collect();
        let once = compact(&data);
        let expected: JsonMap = data.iter().filter(|(_, v)| !is_empty_or_zero(v)).map(|(k, v)| (k.clone(), v.clone())).collect();
        prop_assert_eq!(&once, &expected);
        prop_assert_eq!(compact(&once), once);
    }
}
