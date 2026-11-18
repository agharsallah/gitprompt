//! Real git repositories in temp dirs, isolated from the developer's git config, with a fixed
//! identity and every commit at the date it is given.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// The date of commands that make no commit.
const ANY_DATE: &str = "2026-01-01T00:00:00Z";

/// A repository on branch `main`; `root` is canonical (git reports real paths).
pub struct Repo {
    _dir: TempDir,
    pub root: PathBuf,
}

/// `git -C dir args...` at `date` (author and committer); its trimmed stdout. Panics when it fails.
pub fn git(dir: &Path, date: &str, args: &[&str]) -> String {
    let out = Command::new("git")
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
        .output()
        .expect("git is installed");
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("utf-8").trim().to_owned()
}

/// A canonical `<temp dir>/<name>` (not created) and the temp dir that holds it.
fn place(name: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().canonicalize().expect("canonical").join(name);
    (dir, root)
}

/// An empty repository `name` in a temp dir.
pub fn repo(name: &str) -> Repo {
    let (dir, root) = place(name);
    std::fs::create_dir(&root).expect("mkdir");
    git(&root, ANY_DATE, &["init", "-q", "-b", "main"]);
    Repo { _dir: dir, root }
}

impl Repo {
    /// Commit a change with `message` at `date`; the commit's full hash.
    pub fn commit(&self, message: &str, date: &str) -> String {
        let file = self.root.join("log.txt");
        let before = std::fs::read_to_string(&file).unwrap_or_default();
        std::fs::write(&file, format!("{before}{message}\n")).expect("write");
        git(&self.root, date, &["add", "-A"]);
        git(&self.root, date, &["commit", "-q", "-m", message]);
        git(&self.root, date, &["rev-parse", "HEAD"])
    }

    /// A clone called `name` in a temp dir of its own, without a remote (so its project is called
    /// `name`): the same commits in another project.
    pub fn clone_as(&self, name: &str) -> Repo {
        let (dir, root) = place(name);
        git(&self.root, ANY_DATE, &["clone", "-q", self.path(), root.to_str().expect("utf-8 path")]);
        git(&root, ANY_DATE, &["remote", "remove", "origin"]);
        Repo { _dir: dir, root }
    }

    pub fn path(&self) -> &str {
        self.root.to_str().expect("utf-8 path")
    }
}
