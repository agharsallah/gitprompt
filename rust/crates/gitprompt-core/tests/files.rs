//! `atomic_write`: never a half-written file, never a stray temp file.

mod common;

use common::{entries, mode};
use gitprompt_core::files::atomic_write;
use pretty_assertions::assert_eq;

#[test]
fn writes_the_text_with_the_given_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    atomic_write(&path, "{}\n", 0o600).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}\n");
    assert_eq!(mode(&path), 0o600);
}

#[test]
fn another_mode_is_applied_as_given() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hooks.json");
    atomic_write(&path, "x", 0o644).unwrap();
    assert_eq!(mode(&path), 0o644);
}

#[test]
fn replaces_an_existing_file_and_its_mode_leaving_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(&path, "old content that is longer").unwrap();
    atomic_write(&path, "new", 0o600).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(mode(&path), 0o600);
    assert_eq!(entries(dir.path()), ["prefs.json"]);
}

#[test]
fn creates_missing_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a/b/c.json");
    atomic_write(&path, "x", 0o600).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "x");
}

#[test]
fn writes_unicode_text_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("u.txt");
    atomic_write(&path, "héllo ✓ 日本", 0o600).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "héllo ✓ 日本");
}

#[test]
fn a_failed_rename_keeps_the_target_and_removes_the_temp_file() {
    // The target is a non-empty directory: the rename over it fails after the temp file was written.
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("prefs.json");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep"), "kept").unwrap();

    assert!(atomic_write(&target, "new", 0o600).is_err());

    assert_eq!(std::fs::read_to_string(target.join("keep")).unwrap(), "kept");
    assert_eq!(entries(dir.path()), ["prefs.json"]);
}

#[test]
fn a_parent_that_is_a_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("file"), "x").unwrap();
    assert!(atomic_write(&dir.path().join("file/child.json"), "x", 0o600).is_err());
    assert_eq!(entries(dir.path()), ["file"]);
}
