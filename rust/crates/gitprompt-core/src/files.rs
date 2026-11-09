//! File writes that never leave a half-written file behind.

use std::path::Path;

/// Write `text` to a temporary file next to `path`, fsync it, then rename it over `path` in one step.
/// A crash mid-write leaves the old file intact. The new file gets `mode` (unix permissions);
/// missing parent directories are created. No temporary file is left behind on failure.
pub fn atomic_write(_path: &Path, _text: &str, _mode: u32) -> std::io::Result<()> {
    todo!()
}
