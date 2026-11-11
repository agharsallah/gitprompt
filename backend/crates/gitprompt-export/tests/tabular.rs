//! CSV and JSON of the rows (export/tabular.py): RFC 4180 quoting, formula cells neutralized, the
//! JSON stating its options and round-tripping its rows.
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

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use common::*;
use gitprompt_export::tabular::{columns, csv, json};
use gitprompt_export::{Options, Row};

fn csv_of(rows: Vec<Value>) -> String {
    csv(&export_of(report(vec![]), rows))
}

fn rows(values: Vec<Value>) -> Vec<Row> {
    values.into_iter().map(|v| v.as_object().unwrap().clone()).collect()
}

// --- columns -------------------------------------------------------------------------------------

#[test]
fn columns_are_every_key_of_every_row_in_first_seen_order() {
    assert_eq!(columns(&rows(vec![json!({"b": 1, "a": 2}), json!({"c": 3, "a": 4})])), ["b", "a", "c"]);
}

// --- csv -----------------------------------------------------------------------------------------

#[test]
fn csv_is_a_header_then_one_line_per_row_with_missing_values_empty() {
    assert_eq!(csv_of(vec![json!({"a": 1, "b": "x"}), json!({"b": "y", "c": null})]), "a,b,c\n1,x,\n,y,\n");
}

#[test]
fn csv_writes_numbers_and_booleans_as_json_spells_them() {
    assert_eq!(csv_of(vec![json!({"n": -3, "f": 0.12, "w": 42.0, "t": true, "u": false})]), "n,f,w,t,u\n-3,0.12,42.0,true,false\n");
}

#[test]
fn csv_quotes_commas_quotes_and_line_breaks() {
    assert_eq!(
        csv_of(vec![json!({"a": "x,y", "b": "say \"hi\"", "c": "two\nlines", "d": "cr\rhere"})]),
        "a,b,c,d\n\"x,y\",\"say \"\"hi\"\"\",\"two\nlines\",\"cr\rhere\"\n"
    );
}

#[test]
fn csv_writes_lists_and_objects_as_compact_json() {
    assert_eq!(csv_of(vec![json!({"a": "=HYPERLINK(1)", "b": ["x"]})]), "a,b\n'=HYPERLINK(1),\"[\"\"x\"\"]\"\n");
    assert_eq!(csv_of(vec![json!({"o": {"k": 1, "j": [2, 3]}})]), "o\n\"{\"\"k\"\":1,\"\"j\"\":[2,3]}\"\n");
}

#[test]
fn csv_neutralizes_every_cell_a_spreadsheet_would_run_as_a_formula() {
    assert_eq!(
        csv_of(vec![json!({"eq": "=1+1", "plus": "+1", "minus": "-1", "at": "@SUM(A1)", "tab": "\tx", "cr": "\rx", "fine": "a=b"})]),
        "eq,plus,minus,at,tab,cr,fine\n'=1+1,'+1,'-1,'@SUM(A1),'\tx,\"'\rx\",a=b\n"
    );
}

#[test]
fn csv_header_cells_are_neutralized_too() {
    assert_eq!(csv_of(vec![json!({"=cmd": 1})]), "'=cmd\n1\n");
}

#[test]
fn csv_keeps_non_ascii_text_as_is() {
    assert_eq!(csv_of(vec![json!({"t": "déjà vu ☕"})]), "t\ndéjà vu ☕\n");
}

#[test]
fn csv_of_no_rows_is_an_empty_header_line() {
    assert_eq!(csv_of(vec![]), "\n");
}

// --- json ----------------------------------------------------------------------------------------

fn document() -> String {
    json(&export_of(report(vec![]), vec![json!({"kind": "chapter", "text": "déjà", "n": 1})]))
}

#[test]
fn json_states_head_options_notes_and_rows_in_order() {
    let parsed: Value = serde_json::from_str(&document()).unwrap();
    let keys: Vec<&str> = parsed.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["kind", "title", "subtitle", "generated_at", "options", "notes", "rows"]);
    assert_eq!(
        parsed,
        json!({
            "kind": "session", "title": "A title", "subtitle": "A subtitle", "generated_at": "2026-03-03 10:00",
            "options": Options::default().as_json(), "notes": ["First note.", "Second note."],
            "rows": [{"kind": "chapter", "text": "déjà", "n": 1}],
        })
    );
}

#[test]
fn json_is_indented_by_two_spaces_keeps_non_ascii_and_ends_with_a_newline() {
    let doc = document();
    assert!(doc.starts_with("{\n  \"kind\": \"session\",\n  \"title\": \"A title\","), "{doc}");
    assert!(doc.contains("\"text\": \"déjà\""));
    assert!(doc.ends_with("}\n"));
}

#[test]
fn json_rows_round_trip() {
    let export = export_of(report(vec![]), vec![json!({"a": [1, {"b": null}], "c": 0.5, "d": "x\"y"}), json!({"e": true})]);
    let parsed: Value = serde_json::from_str(&json(&export)).unwrap();
    let back: Vec<Row> = serde_json::from_value(parsed["rows"].clone()).unwrap();
    assert_eq!(back, export.rows);
}

#[test]
fn json_states_the_options_that_made_it() {
    let mut export = export_of(report(vec![]), vec![]);
    export.options = all_open();
    let parsed: Value = serde_json::from_str(&json(&export)).unwrap();
    assert_eq!(parsed["options"], json!({"prompt_text": true, "relative_paths": false, "drop_identity": false, "drop_raw": false}));
}

#[test]
fn the_same_rows_render_the_same_csv_and_json() {
    let export = export_of(report(vec![]), vec![json!({"b": 1, "a": "x"})]);
    assert_eq!((csv(&export), json(&export)), (csv(&export.clone()), json(&export.clone())));
}
