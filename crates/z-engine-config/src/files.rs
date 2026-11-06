//! File primitives shared by every store in this crate: size-capped reads
//! and atomic replacement (sibling temp file, sync, rename).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::ConfigError;

/// Settings, credential, and trust files larger than this are refused.
pub(crate) const MAX_DATA_FILE: usize = 1024 * 1024;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Reads at most `limit` bytes; the flag reports whether the file was longer.
pub(crate) fn read_capped(path: &Path, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    Ok((bytes, truncated))
}

/// A whole UTF-8 data file; `None` when it does not exist. Invalid UTF-8 is
/// a parse error rather than a lossy read, so it can never be written back.
pub(crate) fn read_data_file(path: &Path) -> Result<Option<String>, ConfigError> {
    let (bytes, truncated) = match read_capped(path, MAX_DATA_FILE) {
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ConfigError::io(path, error)),
    };
    if truncated {
        return Err(ConfigError::TooLarge {
            path: path.to_path_buf(),
            limit: MAX_DATA_FILE,
        });
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| ConfigError::parse(path, error))
}

/// Whether anything (even a dangling symlink) exists at `path`.
pub(crate) fn exists(path: &Path) -> Result<bool, ConfigError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ConfigError::io(path, error)),
    }
}

/// Decodes user-authored text, dropping a character cut off by truncation.
pub(crate) fn lossy_text(mut bytes: Vec<u8>) -> String {
    while let Err(error) = std::str::from_utf8(&bytes) {
        if error.error_len().is_some() {
            break;
        }
        bytes.truncate(error.valid_up_to());
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Replaces `path` atomically, creating its directory. `private` files are
/// owner-only on unix; other files keep the permissions of the file they
/// replace.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8], private: bool) -> Result<(), ConfigError> {
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(|error| ConfigError::io(dir, error))?;
    let temp = temp_path(path);
    let result = write_temp(&temp, bytes, private, path)
        .and_then(|()| fs::rename(&temp, path))
        .map_err(|error| ConfigError::io(path, error));
    if result.is_err() {
        if let Err(cleanup) = fs::remove_file(&temp) {
            if cleanup.kind() != io::ErrorKind::NotFound {
                tracing::warn!(path = %temp.display(), error = %cleanup, "could not remove temporary file");
            }
        }
    }
    result
}

fn write_temp(temp: &Path, bytes: &[u8], private: bool, target: &Path) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(temp)?;
    if !private {
        if let Ok(existing) = fs::metadata(target) {
            file.set_permissions(existing.permissions())?;
        }
    }
    file.write_all(bytes)?;
    file.sync_all()
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{name}.{}-{unique}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_read_reports_truncation_and_keeps_whole_characters() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.md");
        fs::write(&path, "abcé").unwrap();
        let (bytes, truncated) = read_capped(&path, 4).unwrap();
        assert!(truncated);
        assert_eq!(lossy_text(bytes), "abc");
        let (bytes, truncated) = read_capped(&path, 5).unwrap();
        assert!(!truncated);
        assert_eq!(lossy_text(bytes), "abcé");
    }

    #[test]
    fn data_files_reject_invalid_utf8_and_missing_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("x.toml");
        assert!(read_data_file(&path).unwrap().is_none());
        fs::write(&path, [0x66, 0xff, 0x66]).unwrap();
        assert!(matches!(
            read_data_file(&path),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp_files() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested/settings.toml");
        write_atomic(&path, b"one", false).unwrap();
        write_atomic(&path, b"two", false).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
        let entries = fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(entries, 1);
    }

    #[cfg(unix)]
    #[test]
    fn private_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("auth.json");
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        write_atomic(&path, b"{}", true).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
