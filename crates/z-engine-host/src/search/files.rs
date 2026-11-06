//! Project file index for `@`-mentions (v1 `list_project_files`, now
//! gitignore-aware and fuzzy): files and directories as root-relative
//! paths, directories with a trailing `/`.

use std::path::Path;
use std::time::SystemTime;

use super::fuzzy::{fold, score};
use super::walk::walker;
use crate::HostError;

/// Never indexed, even when not ignored (v1 skip list).
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    ".harness",
    ".z-engine",
    ".venv",
    "__pycache__",
];
/// A match inside the file name beats a match spread over directories.
const BASENAME_BONUS: i64 = 2000;

#[derive(Debug, Clone)]
pub struct FileIndex {
    entries: Vec<Entry>,
    truncated: bool,
}

#[derive(Debug, Clone)]
struct Entry {
    /// Root-relative path with `/` separators, no trailing slash.
    path: String,
    /// Lowercased `path` for the subsequence prefilter.
    folded: String,
    /// Byte offset of the file name within `path`.
    name_start: usize,
    is_dir: bool,
    modified: Option<SystemTime>,
}

impl FileIndex {
    /// Walks `root` (blocking), keeping at most `max_files` entries.
    pub fn build(root: &Path, max_files: usize) -> Result<FileIndex, HostError> {
        if !root.is_dir() {
            return Err(HostError::NotFound(format!("directory {}", root.display())));
        }
        let mut builder = walker(root);
        // Replaces the walker's own `.git` filter, so `.git` files are
        // excluded here too.
        builder.filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            let skipped = is_dir && SKIP_DIRS.iter().any(|skip| entry.file_name() == *skip);
            !skipped && entry.file_name() != ".git"
        });
        let mut entries = Vec::new();
        let mut truncated = false;
        for entry in builder.build().filter_map(Result::ok) {
            let Ok(rel) = entry.path().strip_prefix(root) else {
                continue;
            };
            if rel.as_os_str().is_empty() {
                continue;
            }
            if entries.len() >= max_files {
                truncated = true;
                break;
            }
            let path = rel.to_string_lossy().replace('\\', "/");
            let name_start = path.rfind('/').map_or(0, |i| i + 1);
            entries.push(Entry {
                folded: path.chars().map(fold).collect(),
                name_start,
                is_dir: entry.file_type().is_some_and(|t| t.is_dir()),
                modified: entry.metadata().ok().and_then(|m| m.modified().ok()),
                path,
            });
        }
        Ok(FileIndex { entries, truncated })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The walk stopped at `max_files`.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Best matches for `query`, best first; an empty query lists the most
    /// recently modified files.
    pub fn search(&self, query: &str, limit: usize) -> Vec<String> {
        let needle: Vec<char> = query
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(fold)
            .collect();
        if needle.is_empty() {
            return self.recent(limit);
        }
        let mut scored: Vec<(i64, &Entry)> = self
            .entries
            .iter()
            .filter(|entry| contains_subsequence(&entry.folded, &needle))
            .filter_map(|entry| Some((rank(&needle, entry)?, entry)))
            .collect();
        scored.sort_by(|(a_score, a), (b_score, b)| {
            b_score
                .cmp(a_score)
                .then_with(|| a.path.len().cmp(&b.path.len()))
                .then_with(|| a.path.cmp(&b.path))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, entry)| entry.display())
            .collect()
    }

    fn recent(&self, limit: usize) -> Vec<String> {
        let mut files: Vec<&Entry> = self.entries.iter().filter(|e| !e.is_dir).collect();
        files.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| a.path.cmp(&b.path))
        });
        files.into_iter().take(limit).map(Entry::display).collect()
    }
}

impl Entry {
    fn display(&self) -> String {
        if self.is_dir {
            format!("{}/", self.path)
        } else {
            self.path.clone()
        }
    }
}

fn rank(needle: &[char], entry: &Entry) -> Option<i64> {
    let full = score(needle, &entry.path)?;
    let name = score(needle, &entry.path[entry.name_start..]).map(|s| s + BASENAME_BONUS);
    Some(name.map_or(full, |name| name.max(full)))
}

fn contains_subsequence(folded: &str, needle: &[char]) -> bool {
    let mut rest = folded.chars();
    needle.iter().all(|wanted| rest.any(|c| c == *wanted))
}
