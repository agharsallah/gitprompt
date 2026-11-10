//! File writes that never leave a half-written file behind.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::random;

/// Write `text` to a temporary file next to `path`, fsync it, then rename it over `path` in one step.
/// A crash mid-write leaves the old file intact. The new file gets `mode` (unix permissions);
/// missing parent directories are created. No temporary file is left behind on failure.
pub fn atomic_write(path: &Path, text: &str, mode: u32) -> io::Result<()> {
    let dir = parent(path);
    std::fs::create_dir_all(dir)?;
    let (tmp, file) = create_temp(dir, path)?;
    let result = fill(file, text, mode).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        // Best effort: the original error is what the caller needs to see.
        let _ = std::fs::remove_file(&tmp);
    }
    result?;
    sync_dir(dir);
    Ok(())
}

/// Write `text` to `path` only if no file is there yet, atomically: a concurrent reader sees either
/// no file or the whole text. Returns `false` (and writes nothing) when the file already exists.
pub(crate) fn create_exclusive(path: &Path, text: &str, mode: u32) -> io::Result<bool> {
    let dir = parent(path);
    std::fs::create_dir_all(dir)?;
    let (tmp, file) = create_temp(dir, path)?;
    // A hard link fails if the target exists, unlike a rename which would replace it.
    let result = fill(file, text, mode).and_then(|()| std::fs::hard_link(&tmp, path));
    let _ = std::fs::remove_file(&tmp); // the link (if any) keeps the data
    match result {
        Ok(()) => {
            sync_dir(dir);
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}

fn parent(path: &Path) -> &Path {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

/// A new, empty, uniquely named temporary file `.<name>.<random>` in `dir`.
fn create_temp(dir: &Path, path: &Path) -> io::Result<(PathBuf, File)> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    loop {
        let tmp = dir.join(format!(".{name}.{}", random::hex_suffix()?));
        match open_new(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(unix)]
fn open_new(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    // Owner-only until `fill` sets the final mode: the text is never readable by others meanwhile.
    OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn open_new(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// Write, set the mode, fsync and close.
fn fill(mut file: File, text: &str, mode: u32) -> io::Result<()> {
    file.write_all(text.as_bytes())?;
    set_mode(&file, mode)?;
    file.sync_all()
}

#[cfg(unix)]
fn set_mode(file: &File, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // Explicit, so the umask doesn't change the requested mode.
    file.set_permissions(std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_file: &File, _mode: u32) -> io::Result<()> {
    Ok(())
}

/// Persist the rename itself (unix). Best effort: the data is already safe in the file.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Err(e) = File::open(dir).and_then(|d| d.sync_all()) {
        tracing::debug!(dir = %dir.display(), error = %e, "could not fsync directory");
    }
    #[cfg(not(unix))]
    let _ = dir;
}
