//! Keeps personal files in `<project>/.z-engine/` out of version control.

use std::path::Path;

use crate::error::ConfigError;
use crate::files::{read_data_file, write_atomic};
use crate::paths::LOCAL_SETTINGS_FILE;

const IGNORED: [&str; 2] = [LOCAL_SETTINGS_FILE, "worktrees/"];

/// Appends missing entries to `dir/.gitignore`; existing lines are kept.
pub(crate) fn ensure_local_gitignore(dir: &Path) -> Result<(), ConfigError> {
    let path = dir.join(".gitignore");
    let mut text = read_data_file(&path)?.unwrap_or_default();
    let missing: Vec<&str> = IGNORED
        .into_iter()
        .filter(|entry| !text.lines().any(|line| line.trim() == *entry))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for entry in missing {
        text.push_str(entry);
        text.push('\n');
    }
    write_atomic(&path, text.as_bytes(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_only_missing_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".gitignore");
        std::fs::write(&path, "cache/\nworktrees/").unwrap();
        ensure_local_gitignore(tmp.path()).unwrap();
        ensure_local_gitignore(tmp.path()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "cache/\nworktrees/\nsettings.local.toml\n");
    }
}
