//! Spilled tool outputs, check logs, and images under `<session>/artifacts/`.
//! Files are named `<hint>-<ulid>.<ext>` and never overwritten. Writes are
//! flushed, not fsynced: a missing artifact must be reported as missing
//! evidence by its reader.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::StoreError;

const MAX_HINT_CHARS: usize = 48;
const MAX_EXT_CHARS: usize = 16;

#[derive(Debug, Clone)]
pub struct Artifacts {
    dir: PathBuf,
    /// Why writes are refused: the session id was not a valid path segment.
    rejected: Option<String>,
}

impl Artifacts {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            rejected: None,
        }
    }

    pub(crate) fn rejected(dir: &Path, reason: String) -> Self {
        Self {
            dir: dir.to_path_buf(),
            rejected: Some(reason),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Write `text` to a new file. `stem_hint` is reduced to `[a-z0-9_-]`
    /// and `ext` to ASCII alphanumerics (`txt` when empty).
    pub fn write_text(
        &self,
        stem_hint: &str,
        ext: &str,
        text: &str,
    ) -> Result<PathBuf, StoreError> {
        self.write(stem_hint, ext, "txt", text.as_bytes())
    }

    /// Like `write_text`; the extension defaults to `bin`.
    pub fn write_bytes(
        &self,
        stem_hint: &str,
        ext: &str,
        bytes: &[u8],
    ) -> Result<PathBuf, StoreError> {
        self.write(stem_hint, ext, "bin", bytes)
    }

    fn write(
        &self,
        stem_hint: &str,
        ext: &str,
        default_ext: &str,
        bytes: &[u8],
    ) -> Result<PathBuf, StoreError> {
        if let Some(reason) = &self.rejected {
            return Err(StoreError::Invalid(reason.clone()));
        }
        fs::create_dir_all(&self.dir).map_err(|error| StoreError::io(&self.dir, error))?;
        let unique = ulid::Ulid::new().to_string().to_lowercase();
        let name = format!(
            "{}-{unique}.{}",
            stem(stem_hint),
            extension(ext, default_ext)
        );
        let path = self.dir.join(name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| StoreError::io(&path, error))?;
        file.write_all(bytes)
            .and_then(|()| file.flush())
            .map_err(|error| StoreError::io(&path, error))?;
        Ok(path)
    }
}

fn stem(hint: &str) -> String {
    let mut out = String::new();
    for c in hint.chars().map(|c| c.to_ascii_lowercase()) {
        let c = if c.is_ascii_alphanumeric() || c == '_' {
            c
        } else {
            '-'
        };
        if c == '-' && (out.is_empty() || out.ends_with('-')) {
            continue;
        }
        if out.len() == MAX_HINT_CHARS {
            break;
        }
        out.push(c);
    }
    let trimmed = out.trim_end_matches('-');
    if trimmed.is_empty() {
        "artifact".to_string()
    } else {
        trimmed.to_string()
    }
}

fn extension(ext: &str, default: &str) -> String {
    let clean: String = ext
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(MAX_EXT_CHARS)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if clean.is_empty() {
        default.to_string()
    } else {
        clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_keep_only_safe_characters() {
        assert_eq!(stem("Bash output"), "bash-output");
        assert_eq!(stem("../../etc/passwd"), "etc-passwd");
        assert_eq!(stem("  --  "), "artifact");
        assert_eq!(stem(&"x".repeat(100)).len(), MAX_HINT_CHARS);
        assert_eq!(stem("cargo_test#1"), "cargo_test-1");
    }

    #[test]
    fn extensions_are_alphanumeric_with_defaults() {
        assert_eq!(extension(".LOG", "txt"), "log");
        assert_eq!(extension("", "bin"), "bin");
        assert_eq!(extension("../", "txt"), "txt");
    }
}
