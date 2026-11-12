//! The workspace as Cargo sees it (`cargo metadata --no-deps`): each member, its source folders and
//! its dependencies.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Normal,
    Build,
    Dev,
}

#[derive(Debug)]
pub struct Dep {
    pub name: String,
    pub kind: Kind,
}

#[derive(Debug)]
pub struct Package {
    pub name: String,
    /// Folders holding a library or binary root (`lib.rs`, `main.rs`).
    pub src_dirs: Vec<PathBuf>,
    pub deps: Vec<Dep>,
}

impl Package {
    /// Dependencies that ship with the crate (not dev-dependencies).
    pub fn shipped(&self) -> impl Iterator<Item = &Dep> {
        self.deps.iter().filter(|dep| dep.kind != Kind::Dev)
    }
}

/// `backend/`, the workspace root.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// `path` relative to the workspace root, for messages.
pub fn shown(path: &Path) -> String {
    path.strip_prefix(root()).unwrap_or(path).display().to_string()
}

pub fn packages() -> Vec<Package> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps", "--manifest-path"])
        .arg(root().join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(output.status.success(), "cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr));
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    metadata["packages"].as_array().unwrap().iter().map(package).collect()
}

fn package(value: &Value) -> Package {
    Package {
        name: text(&value["name"]),
        src_dirs: src_dirs(value),
        deps: value["dependencies"].as_array().unwrap().iter().map(dep).collect(),
    }
}

fn src_dirs(value: &Value) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = value["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|target| target["kind"].as_array().unwrap().iter().any(|kind| kind == "lib" || kind == "bin"))
        .map(|target| Path::new(&text(&target["src_path"])).parent().unwrap().to_path_buf())
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

fn dep(value: &Value) -> Dep {
    let kind = match value["kind"].as_str() {
        Some("dev") => Kind::Dev,
        Some("build") => Kind::Build,
        _ => Kind::Normal,
    };
    Dep { name: text(&value["name"]), kind }
}

fn text(value: &Value) -> String {
    value.as_str().unwrap().to_owned()
}
