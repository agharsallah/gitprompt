//! The database file is private: its directory is created with mode 0700, the file (and its WAL
//! companions) get mode 0600.

use std::io;
use std::path::Path;

/// Create the parent directory (0700) and an empty database file (0600) when missing, so SQLite's
/// `-wal` and `-shm` files, which copy the database file's mode, are private from the start.
#[cfg(unix)]
pub(crate) fn prepare(path: &Path) -> io::Result<()> {
    use std::fs::{DirBuilder, OpenOptions};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty() && !dir.exists()) {
        DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    match OpenOptions::new().write(true).create_new(true).mode(0o600).open(path) {
        Err(e) if e.kind() != io::ErrorKind::AlreadyExists => Err(e),
        _ => Ok(()),
    }
}

#[cfg(not(unix))]
pub(crate) fn prepare(path: &Path) -> io::Result<()> {
    match path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        Some(dir) => std::fs::create_dir_all(dir),
        None => Ok(()),
    }
}

/// Mode 0600 for the database file and its `-wal`/`-shm` files (an existing database may have been
/// created with looser modes).
#[cfg(unix)]
pub(crate) fn make_private(path: &Path) -> io::Result<()> {
    use std::fs::{Permissions, set_permissions};
    use std::os::unix::fs::PermissionsExt;

    set_permissions(path, Permissions::from_mode(0o600))?;
    for suffix in ["-wal", "-shm"] {
        let mut companion = path.as_os_str().to_owned();
        companion.push(suffix);
        match set_permissions(&companion, Permissions::from_mode(0o600)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn make_private(_path: &Path) -> io::Result<()> {
    Ok(())
}
