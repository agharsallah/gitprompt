//! Opening folders and revealing files in the OS file manager, behind [`Opener`] so tests never
//! open a Finder window.

use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::error::CommandError;

/// Shows paths to the user.
pub trait Opener: Send + Sync {
    /// Open a folder in the file manager.
    fn open(&self, path: &Path) -> io::Result<()>;
    /// Show a file selected in its folder.
    fn reveal(&self, path: &Path) -> io::Result<()>;
}

/// The platform's file manager: `open` / `open -R` (macOS), `explorer` / `explorer /select,`
/// (Windows), `xdg-open` on the folder (elsewhere; there is no standard "select").
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemOpener;

impl Opener for SystemOpener {
    fn open(&self, path: &Path) -> io::Result<()> {
        run(open_command(path))
    }

    fn reveal(&self, path: &Path) -> io::Result<()> {
        run(reveal_command(path))
    }
}

fn open_command(path: &Path) -> Command {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let mut command = Command::new(program);
    command.arg(path);
    command
}

fn reveal_command(path: &Path) -> Command {
    if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg("-R").arg(path);
        command
    } else if cfg!(windows) {
        let mut command = Command::new("explorer");
        command.arg(format!("/select,{}", path.display()));
        command
    } else {
        open_command(path.parent().unwrap_or(path))
    }
}

/// Start the file manager without waiting for it (it may stay open).
fn run(mut command: Command) -> io::Result<()> {
    command.spawn().map(drop)
}

/// The backup called `name` in `backups`: a plain file name (no separators, no `..`), and the file
/// must exist (404 otherwise), so a command can't be pointed at an arbitrary path.
pub(crate) fn backup_path(backups: &Path, name: &str) -> Result<PathBuf, CommandError> {
    let mut components = Path::new(name).components();
    let plain = matches!((components.next(), components.next()), (Some(Component::Normal(_)), None));
    if !plain {
        return Err(CommandError::invalid(format!("{name:?} is not a backup file name")));
    }
    let path = backups.join(name);
    if path.is_file() { Ok(path) } else { Err(CommandError::not_found(format!("no backup called {name:?}"))) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_backup_is_a_file_in_the_backups_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json.bak"), b"{}").unwrap();
        assert_eq!(backup_path(dir.path(), "settings.json.bak").unwrap(), dir.path().join("settings.json.bak"));
    }

    #[test]
    fn a_missing_backup_is_404() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(backup_path(dir.path(), "gone.bak").unwrap_err().status, 404);
    }

    #[test]
    fn paths_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["../token", "/etc/passwd", "a/b", "", "."] {
            assert_eq!(backup_path(dir.path(), name).unwrap_err().status, 400, "{name}");
        }
    }

    #[test]
    fn the_folder_is_not_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        assert_eq!(backup_path(dir.path(), "sub").unwrap_err().status, 404);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_reveals_with_open_r() {
        let command = reveal_command(Path::new("/tmp/x"));
        assert_eq!(command.get_program(), "open");
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["-R", "/tmp/x"]);
    }
}
