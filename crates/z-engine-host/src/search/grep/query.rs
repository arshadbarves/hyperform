//! The `Grep` request and result shapes (Claude Code's Grep tool inputs).

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GrepMode {
    /// `path:line:text` lines, with context lines as `path-line-text`.
    Content,
    /// One matching file path per line.
    #[default]
    FilesWithMatches,
    /// `path:count` per matching file.
    Count,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepQuery {
    /// Rust `regex` syntax (what ripgrep uses by default).
    pub pattern: String,
    /// File or directory to search, relative to the root or absolute.
    pub path: Option<PathBuf>,
    /// ripgrep-style glob filter (`*.rs`, `src/**/*.ts`, `!*.min.js`).
    pub glob: Option<String>,
    /// ripgrep `--type` name (`rust`, `js`, `py`, ...).
    pub file_type: Option<String>,
    pub case_insensitive: bool,
    /// Patterns may span lines and `.` matches newlines.
    pub multiline: bool,
    /// Context lines before / after each match (content mode).
    pub before: usize,
    pub after: usize,
    pub mode: GrepMode,
    /// Show line numbers in content mode.
    pub line_numbers: bool,
    /// Keep at most this many output lines (content) or entries.
    pub head_limit: Option<usize>,
    /// Skip this many output lines/entries before `head_limit` applies.
    pub offset: usize,
}

impl Default for GrepQuery {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            path: None,
            glob: None,
            file_type: None,
            case_insensitive: false,
            multiline: false,
            before: 0,
            after: 0,
            mode: GrepMode::default(),
            line_numbers: true,
            head_limit: None,
            offset: 0,
        }
    }
}

impl GrepQuery {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrepEngine {
    Ripgrep,
    Builtin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepResult {
    /// ripgrep-style output with paths relative to the search root, after
    /// `offset`/`head_limit`; lines are joined with `\n`.
    pub text: String,
    /// Matching lines (content), summed counts (count), or files (files).
    pub matches: usize,
    /// Files with at least one match.
    pub files: usize,
    /// Output continued past `head_limit`, or a safety cap was hit.
    pub truncated: bool,
    pub engine: GrepEngine,
}
