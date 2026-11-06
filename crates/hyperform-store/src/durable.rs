//! Durability primitives: atomic replacement, directory sync, and noticing
//! a log file that was deleted or replaced behind an open handle.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::error::StoreError;

/// Replace `path` with `bytes`: write a sibling temp file, fsync it, rename
/// it over the target, then fsync the directory. Readers see the old or the
/// new content, never a mix.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let dir = parent_dir(path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let temp = dir.join(format!(".{name}.tmp-{}", ulid::Ulid::new()));
    let written = write_new_file(&temp, bytes)
        .and_then(|()| fs::rename(&temp, path).map_err(|error| StoreError::io(path, error)));
    if let Err(error) = written {
        remove_leftover(&temp);
        return Err(error);
    }
    sync_dir(dir)
}

/// Create `path`, which must not exist yet, holding `bytes`, fsynced.
pub(crate) fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| StoreError::io(path, error))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| StoreError::io(path, error))
}

/// Fsync a directory so entries created or renamed in it survive a crash.
/// Directory handles cannot be synced portably outside Unix.
#[cfg_attr(not(unix), allow(unused_variables))]
pub(crate) fn sync_dir(dir: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(|error| StoreError::io(dir, error))?;
    Ok(())
}

/// Fail unless `path` still names the file behind `file`, so records are
/// never acknowledged into a deleted or replaced log.
#[cfg_attr(not(unix), allow(unused_variables))]
pub(crate) fn ensure_same_file(file: &File, path: &Path) -> Result<(), StoreError> {
    let current = fs::metadata(path).map_err(|error| StoreError::open(path, error))?;
    if !current.is_file() {
        return Err(StoreError::Invalid(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file
            .metadata()
            .map_err(|error| StoreError::io(path, error))?;
        if opened.dev() != current.dev() || opened.ino() != current.ino() {
            return Err(StoreError::Invalid(format!(
                "{} was replaced outside the session store",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Last modification time in epoch milliseconds.
pub(crate) fn modified_ms(path: &Path) -> Option<u64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    u64::try_from(modified.duration_since(UNIX_EPOCH).ok()?.as_millis()).ok()
}

fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn remove_leftover(temp: &Path) {
    match fs::remove_file(temp) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            tracing::warn!(path = %temp.display(), %error, "could not remove temporary file");
        }
    }
}
