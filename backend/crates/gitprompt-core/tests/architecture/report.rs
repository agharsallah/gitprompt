//! Writes the architecture report: `backend/target/architecture/report.json` and a self-contained
//! page, `index.html` (the template `report.html` with the JSON inlined; no network, no libraries).

use std::path::PathBuf;

use crate::metrics::Report;
use crate::workspace::root;

const TEMPLATE: &str = include_str!("report.html");
const SLOT: &str = "__REPORT__";

/// Writes both files and returns the page's path.
pub fn write(report: &Report) -> PathBuf {
    let dir = root().join("target/architecture");
    std::fs::create_dir_all(&dir).unwrap();
    let json = serde_json::to_string_pretty(report).unwrap();
    std::fs::write(dir.join("report.json"), &json).unwrap();
    // Inside <script>, `</` could close the element early.
    let page = TEMPLATE.replace(SLOT, &json.replace("</", "<\\/"));
    let path = dir.join("index.html");
    std::fs::write(&path, page).unwrap();
    path
}

#[test]
fn the_template_has_one_slot_for_the_data() {
    assert_eq!(TEMPLATE.matches(SLOT).count(), 1);
}
