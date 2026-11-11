//! Shared helpers for gitprompt-core's integration tests. Every path lives under a temp dir: no test
//! touches the real home, and no test mutates the process environment.
#![allow(dead_code, reason = "each test binary uses a different subset of these helpers")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gitprompt_core::{Json, Settings};
use proptest::prelude::*;
use tempfile::TempDir;

/// Settings rooted in a fresh temp dir: data in `<tmp>/gitprompt`, `~` in `<tmp>/home`.
pub struct Sandbox {
    pub dir: TempDir,
    pub settings: Settings,
}

pub fn sandbox() -> Sandbox {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::new(dir.path().join("gitprompt"), dir.path().join("home"));
    Sandbox { dir, settings }
}

impl Sandbox {
    /// `<home>/sources/<name>` with `text`, creating the directory.
    pub fn user_spec(&self, name: &str, text: &str) -> PathBuf {
        let dir = self.settings.user_sources();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }
}

/// The smallest valid spec (no defaults needed).
pub const MINIMAL: &str = "id: mytool\nname: My tool\ndetect: [~/.mytool]\ninstall: {file: ~/.mytool/hooks.json, merge: {}}\n";

pub fn bundled(name: &str) -> &'static str {
    gitprompt_core::spec::BUNDLED.iter().find(|(n, _)| *n == name).map(|(_, text)| *text).unwrap()
}

pub fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// File names in a directory, sorted.
pub fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

// --- git ------------------------------------------------------------------------------------

pub const DATE: &str = "2026-01-01T10:00:00+02:00";

/// `git -C dir args...` isolated from the developer's git config, with a fixed identity and date.
pub fn git_output(dir: &Path, date: &str, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("git is installed")
}

/// Run git and return its trimmed stdout; panics with stderr when it fails.
pub fn git(dir: &Path, args: &[&str]) -> String {
    git_dated(dir, DATE, args)
}

pub fn git_dated(dir: &Path, date: &str, args: &[&str]) -> String {
    let out = git_output(dir, date, args);
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A git repository in a temp dir with branch `main` and no commits. `root` is canonical (git reports
/// real paths; macOS temp dirs sit behind the /var -> /private/var symlink).
pub struct Repo {
    pub dir: TempDir,
    pub root: PathBuf,
}

pub fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap().join("repo");
    std::fs::create_dir(&root).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    Repo { dir, root }
}

impl Repo {
    pub fn write(&self, name: &str, text: &str) {
        let path = self.root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub fn git(&self, args: &[&str]) -> String {
        git(&self.root, args)
    }

    /// Write `name`, stage everything and commit with `message` at `date`.
    pub fn commit(&self, name: &str, text: &str, message: &str, date: &str) {
        self.write(name, text);
        self.git(&["add", "-A"]);
        git_dated(&self.root, date, &["commit", "-q", "-m", message]);
    }

    pub fn head(&self, short: usize) -> String {
        self.git(&["rev-parse", "HEAD"])[..short].to_string()
    }

    pub fn short_head(&self) -> String {
        self.git(&["rev-parse", "--short", "HEAD"])
    }

    /// A sibling directory path (not created) next to the repository, canonical.
    pub fn sibling(&self, name: &str) -> PathBuf {
        self.root.parent().unwrap().join(name)
    }
}

// --- JSON strategies ------------------------------------------------------------------------

/// Arbitrary JSON values (object keys have no dots or brackets, so flattened paths stay readable).
pub fn arb_json() -> impl Strategy<Value = Json> {
    let leaf = prop_oneof![
        Just(Json::Null),
        any::<bool>().prop_map(Json::Bool),
        any::<i64>().prop_map(Json::from),
        "\\PC{0,24}".prop_map(Json::String),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Json::Array),
            prop::collection::btree_map("[a-z_]{1,6}", inner, 0..5).prop_map(|m| Json::Object(m.into_iter().collect())),
        ]
    })
}
