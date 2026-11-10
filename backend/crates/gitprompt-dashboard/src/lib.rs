//! The GitPrompt dashboard (the SvelteKit static build in `web/build`), compiled into the binary.
//!
//! * **Release builds** embed every file of `web/build` at compile time (`build.rs` re-embeds after
//!   a new `pnpm --dir web build`). A build made before the dashboard was built embeds nothing:
//!   [`is_built`] is `false` and the server answers with its "not built" page.
//! * **Debug builds** read the files from [`BUILD_DIR`] at run time, so a new web build is picked
//!   up without recompiling.
//!
//! Paths are relative to the build folder, `/`-separated, without a leading slash
//! (`index.html`, `_app/immutable/entry/start.abc.js`).

use std::borrow::Cow;
use std::path::{Component, PathBuf};

use rust_embed::Embed;

/// The folder the files come from: read at run time in debug builds, embedded in release builds.
pub const BUILD_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../web/build");

/// [`BUILD_DIR`] with its `..` resolved (lexically), for display.
pub fn build_dir() -> PathBuf {
    let mut out = PathBuf::new();
    for component in std::path::Path::new(BUILD_DIR).components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Whether this binary reads [`BUILD_DIR`] at run time (debug build) instead of carrying the files.
pub const READS_FROM_DISK: bool = cfg!(debug_assertions);

/// The page every build has; its absence means the dashboard was not built.
pub const INDEX: &str = "index.html";

#[derive(Embed)]
#[folder = "../../../web/build"]
#[allow_missing = true]
struct Build;

/// The bytes of a dashboard file, `None` when there is none (or `path` names a folder).
pub fn get(path: &str) -> Option<Cow<'static, [u8]>> {
    Build::get(path).map(|file| file.data)
}

/// Whether the dashboard was built (it has an [`INDEX`]).
pub fn is_built() -> bool {
    Build::get(INDEX).is_some()
}

/// How many files the dashboard has (0 when it was not built).
pub fn file_count() -> usize {
    Build::iter().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn being_built_means_having_an_index_page() {
        assert_eq!(is_built(), get(INDEX).is_some());
        if is_built() {
            assert!(file_count() > 0);
        }
    }

    #[test]
    fn the_build_dir_is_web_build_of_the_repository() {
        let dir = build_dir();
        assert!(dir.ends_with("web/build"), "{}", dir.display());
        assert!(!dir.components().any(|c| c == Component::ParentDir));
    }

    #[test]
    fn folders_and_unknown_files_are_none() {
        assert!(get("no/such/file.js").is_none());
        assert!(get("_app").is_none());
    }

    #[test]
    fn paths_leaving_the_build_folder_are_none() {
        assert!(get("../package.json").is_none());
        assert!(get("../../Cargo.toml").is_none());
    }
}
