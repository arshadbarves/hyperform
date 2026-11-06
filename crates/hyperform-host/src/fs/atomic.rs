//! Crash-safe whole-file replacement: write a temp sibling, flush it, then
//! rename it over the target. A crash mid-write never leaves the target
//! truncated. This is not a multi-file transaction.

use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::HostError;
use crate::blocking::run_blocking;

/// Replaces `path` with `bytes` atomically, creating parent directories.
/// Existing unix permissions are preserved and a symlinked target is
/// written through, so the link survives.
pub async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), HostError> {
    let path = path.to_path_buf();
    let bytes = bytes.to_vec();
    run_blocking(move || atomic_write_sync(&path, &bytes)).await
}

/// Blocking variant of [`atomic_write`] for synchronous callers.
pub fn atomic_write_sync(path: &Path, bytes: &[u8]) -> Result<(), HostError> {
    let target = write_target(path);
    let parent = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name = target
        .file_name()
        .ok_or_else(|| HostError::Invalid(format!("{} has no file name", target.display())))?;
    std::fs::create_dir_all(&parent).map_err(|e| HostError::io(&parent, e))?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        ulid::Ulid::new()
    ));
    let written = write_temp(&tmp, bytes, &target).and_then(|()| std::fs::rename(&tmp, &target));
    if let Err(e) = written {
        discard(&tmp);
        return Err(HostError::io(&target, e));
    }
    sync_dir(&parent);
    Ok(())
}

/// Writes through a symlink to its target so the link itself survives.
fn write_target(path: &Path) -> PathBuf {
    let is_link = std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink());
    if is_link {
        if let Ok(resolved) = std::fs::canonicalize(path) {
            return resolved;
        }
    }
    path.to_path_buf()
}

fn write_temp(tmp: &Path, bytes: &[u8], target: &Path) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp)?;
    file.write_all(bytes)?;
    preserve_mode(&file, target)?;
    file.sync_all()
}

#[cfg(unix)]
fn preserve_mode(file: &File, target: &Path) -> std::io::Result<()> {
    match std::fs::metadata(target) {
        Ok(meta) => file.set_permissions(meta.permissions()),
        Err(_) => Ok(()),
    }
}

#[cfg(not(unix))]
fn preserve_mode(_file: &File, _target: &Path) -> std::io::Result<()> {
    Ok(())
}

fn discard(tmp: &Path) {
    if let Err(e) = std::fs::remove_file(tmp) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(path = %tmp.display(), error = %e, "could not remove temp file");
        }
    }
}

/// Makes the rename durable on unix; best-effort because the replacement
/// itself already succeeded.
#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Err(e) = File::open(dir).and_then(|d| d.sync_all()) {
        tracing::debug!(dir = %dir.display(), error = %e, "directory fsync failed");
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}
