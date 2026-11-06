//! Language-server results in the model's coordinates: absolute paths,
//! 1-based lines, 1-based columns counted in characters.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    /// The trimmed source line at `line`, when the file is readable.
    pub snippet: Option<String>,
}

/// An LSP `SymbolKind` number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolKind(pub u32);

impl SymbolKind {
    pub fn label(self) -> &'static str {
        const LABELS: [&str; 26] = [
            "file",
            "module",
            "namespace",
            "package",
            "class",
            "method",
            "property",
            "field",
            "constructor",
            "enum",
            "interface",
            "function",
            "variable",
            "constant",
            "string",
            "number",
            "boolean",
            "array",
            "object",
            "key",
            "null",
            "enum member",
            "struct",
            "event",
            "operator",
            "type parameter",
        ];
        (self.0 as usize)
            .checked_sub(1)
            .and_then(|index| LABELS.get(index))
            .copied()
            .unwrap_or("symbol")
    }
}

/// A node of a file's symbol tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolNode {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    /// Position of the symbol's name.
    pub line: u32,
    pub column: u32,
    /// Full extent, including the body.
    pub start_line: u32,
    pub end_line: u32,
    pub children: Vec<SymbolNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub container: Option<String>,
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// Markdown.
    pub contents: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallDirection {
    /// Who calls the function.
    Incoming,
    /// What the function calls.
    Outgoing,
}

/// A function in a call hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallItem {
    pub name: String,
    pub kind: SymbolKind,
    pub detail: Option<String>,
    /// Position of the function's name.
    pub location: Location,
}

/// One edge of a call hierarchy. Incoming: `item` calls the queried
/// function at `sites` (inside `item`). Outgoing: the queried function
/// calls `item` at `sites` (inside the queried function).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEdge {
    pub item: CallItem,
    pub sites: Vec<Location>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Information,
    Hint,
}

impl Severity {
    /// LSP severity number; unknown or missing values count as errors.
    pub fn from_lsp(value: Option<u64>) -> Self {
        match value {
            Some(2) => Self::Warning,
            Some(3) => Self::Information,
            Some(4) => Self::Hint,
            _ => Self::Error,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Information => "info",
            Self::Hint => "hint",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub severity: Severity,
    pub message: String,
    pub code: Option<String>,
    pub source: Option<String>,
}

/// Diagnostics of one file. `fresh` is false when the server published
/// nothing for the current content within the wait: the list may be stale
/// or incomplete, not proof of a clean file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiagnostics {
    pub path: PathBuf,
    pub diagnostics: Vec<Diagnostic>,
    pub fresh: bool,
}

/// A proposed edit; nothing is applied. Positions are 1-based characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEditPlan {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEdits {
    pub path: PathBuf,
    /// Sorted by position; edits never overlap.
    pub edits: Vec<TextEditPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEditPlan {
    /// Sorted by path.
    pub files: Vec<FileEdits>,
}

impl WorkspaceEditPlan {
    pub fn edit_count(&self) -> usize {
        self.files.iter().map(|file| file.edits.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_cover_known_and_unknown_values() {
        assert_eq!(SymbolKind(12).label(), "function");
        assert_eq!(SymbolKind(23).label(), "struct");
        assert_eq!(SymbolKind(26).label(), "type parameter");
        assert_eq!(SymbolKind(0).label(), "symbol");
        assert_eq!(SymbolKind(99).label(), "symbol");
        assert_eq!(Severity::from_lsp(Some(2)), Severity::Warning);
        assert_eq!(Severity::from_lsp(None), Severity::Error);
    }
}
