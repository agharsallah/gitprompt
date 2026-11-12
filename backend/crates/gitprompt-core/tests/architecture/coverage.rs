//! Line coverage from the last `just cov`: `backend/target/llvm-cov/summary.json` (the LLVM coverage
//! export, `--summary-only`). Optional: without it the report shows the architecture alone.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use serde_json::Value;

use crate::workspace::root;

/// Covered and coverable lines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Lines {
    pub covered: u64,
    pub count: u64,
}

impl Lines {
    pub fn add(self, other: Lines) -> Lines {
        Lines { covered: self.covered + other.covered, count: self.count + other.count }
    }
}

#[derive(Debug)]
pub struct Coverage {
    pub files: BTreeMap<PathBuf, Lines>,
    /// When the summary was written (seconds since the epoch), so the page can say how old it is.
    pub measured_at: u64,
}

impl Coverage {
    /// Lines of every measured file under `dir`; `None` when none is (a crate the run excluded).
    pub fn under(&self, dir: &Path) -> Option<Lines> {
        let mut found = self.files.iter().filter(|(path, _)| path.starts_with(dir)).map(|(_, lines)| *lines).peekable();
        found.peek()?;
        Some(found.fold(Lines::default(), Lines::add))
    }

    pub fn of(&self, file: &Path) -> Option<Lines> {
        self.files.get(file).copied()
    }
}

/// The summary `just cov` wrote.
pub fn summary_path() -> PathBuf {
    root().join("target/llvm-cov/summary.json")
}

/// The line-by-line HTML report `just cov` wrote, relative to the architecture page.
pub const HTML_FROM_REPORT: &str = "../llvm-cov/html/index.html";

pub fn load() -> Option<Coverage> {
    let path = summary_path();
    let text = std::fs::read_to_string(&path).ok()?;
    let measured_at = std::fs::metadata(&path).ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(Coverage { files: parse(&serde_json::from_str(&text).ok()?), measured_at })
}

fn parse(export: &Value) -> BTreeMap<PathBuf, Lines> {
    let files = export["data"].as_array().into_iter().flatten().flat_map(|data| data["files"].as_array().into_iter().flatten());
    files
        .filter_map(|file| {
            let lines = &file["summary"]["lines"];
            let path = PathBuf::from(file["filename"].as_str()?);
            let path = path.canonicalize().unwrap_or(path);
            Some((path, Lines { covered: lines["covered"].as_u64()?, count: lines["count"].as_u64()? }))
        })
        .collect()
}

#[test]
fn the_llvm_export_is_read_per_file() {
    let export = serde_json::json!({"data": [{"files": [
        {"filename": "/x/src/a.rs", "summary": {"lines": {"count": 10, "covered": 7, "percent": 70.0}}},
        {"filename": "/x/src/b/c.rs", "summary": {"lines": {"count": 4, "covered": 4, "percent": 100.0}}}
    ]}]});
    let coverage = Coverage { files: parse(&export), measured_at: 0 };
    assert_eq!(coverage.under(Path::new("/x/src")), Some(Lines { covered: 11, count: 14 }));
    assert_eq!(coverage.of(Path::new("/x/src/b/c.rs")), Some(Lines { covered: 4, count: 4 }));
    assert_eq!(coverage.under(Path::new("/y")), None);
}
