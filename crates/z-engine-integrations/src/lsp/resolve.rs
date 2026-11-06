//! Raw server results to model coordinates. UTF-16 offsets become character
//! columns using the text the server saw: the synced document when open,
//! otherwise the file on disk (each file read once per operation). When a
//! file cannot be read the raw offset is kept and no snippet is shown.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::convert::{
    LspRange, RawCall, RawCallItem, RawDiagnostic, RawLocation, RawSymbol, RawWorkspaceSymbol,
};
use super::documents::Documents;
use super::position::{LspPosition, char_index};
use super::types::{
    CallEdge, CallItem, Diagnostic, FileEdits, Location, Severity, SymbolKind, SymbolNode,
    TextEditPlan, WorkspaceEditPlan, WorkspaceSymbol,
};

const SNIPPET_CHARS: usize = 200;

/// Line starts of one text.
#[derive(Debug)]
struct LineIndex {
    text: Arc<str>,
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: Arc<str>) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
        Self { text, starts }
    }

    fn line(&self, index: usize) -> Option<&str> {
        let start = *self.starts.get(index)?;
        let end = self
            .starts
            .get(index + 1)
            .map_or(self.text.len(), |next| next - 1);
        let line = &self.text[start..end];
        Some(line.strip_suffix('\r').unwrap_or(line))
    }
}

pub(crate) struct Texts<'a> {
    documents: &'a Documents,
    files: HashMap<PathBuf, Option<LineIndex>>,
}

impl<'a> Texts<'a> {
    pub(crate) fn new(documents: &'a Documents) -> Self {
        Self {
            documents,
            files: HashMap::new(),
        }
    }

    pub(crate) async fn load(&mut self, path: &Path) {
        if self.files.contains_key(path) {
            return;
        }
        let text = match self.documents.text(path) {
            Some(text) => Some(text),
            None => tokio::fs::read_to_string(path).await.ok().map(Arc::from),
        };
        self.files
            .insert(path.to_path_buf(), text.map(LineIndex::new));
    }

    pub(crate) async fn load_all<'p>(&mut self, paths: impl IntoIterator<Item = &'p PathBuf>) {
        for path in paths {
            self.load(path).await;
        }
    }

    fn line(&self, path: &Path, index: u32) -> Option<&str> {
        self.files.get(path)?.as_ref()?.line(index as usize)
    }

    fn point(&self, path: &Path, position: LspPosition) -> (u32, u32) {
        let column = match self.line(path, position.line) {
            Some(line) => u32::try_from(char_index(line, position.character)).unwrap_or(u32::MAX),
            None => position.character,
        };
        (position.line.saturating_add(1), column.saturating_add(1))
    }

    fn span(&self, path: &Path, range: LspRange) -> (u32, u32, u32, u32) {
        let (line, column) = self.point(path, range.start);
        let (end_line, end_column) = self.point(path, range.end);
        (line, column, end_line, end_column)
    }

    pub(crate) fn location(&self, raw: &RawLocation) -> Location {
        self.location_at(&raw.path, raw.range)
    }

    fn location_at(&self, path: &Path, range: LspRange) -> Location {
        let (line, column, end_line, end_column) = self.span(path, range);
        let snippet = self.line(path, range.start.line).map(|text| {
            let trimmed = text.trim();
            let mut snippet: String = trimmed.chars().take(SNIPPET_CHARS).collect();
            if trimmed.chars().count() > SNIPPET_CHARS {
                snippet.push('…');
            }
            snippet
        });
        Location {
            path: path.to_path_buf(),
            line,
            column,
            end_line,
            end_column,
            snippet,
        }
    }

    pub(crate) fn symbols(&self, path: &Path, raw: &[RawSymbol]) -> Vec<SymbolNode> {
        raw.iter()
            .map(|symbol| {
                let (line, column) = self.point(path, symbol.selection_range.start);
                SymbolNode {
                    name: symbol.name.clone(),
                    detail: symbol.detail.clone().filter(|detail| !detail.is_empty()),
                    kind: SymbolKind(symbol.kind),
                    line,
                    column,
                    start_line: symbol.range.start.line.saturating_add(1),
                    end_line: symbol.range.end.line.saturating_add(1),
                    children: self.symbols(path, &symbol.children),
                }
            })
            .collect()
    }

    pub(crate) fn workspace_symbol(&self, raw: &RawWorkspaceSymbol) -> WorkspaceSymbol {
        WorkspaceSymbol {
            name: raw.name.clone(),
            kind: SymbolKind(raw.kind),
            container: raw.container.clone().filter(|c| !c.is_empty()),
            location: self.location(&raw.location),
        }
    }

    pub(crate) fn call_item(&self, raw: &RawCallItem) -> CallItem {
        CallItem {
            name: raw.name.clone(),
            kind: SymbolKind(raw.kind),
            detail: raw.detail.clone().filter(|detail| !detail.is_empty()),
            location: self.location(&raw.location),
        }
    }

    /// `sites_in` is the file the call ranges refer to: the caller's for
    /// incoming calls, the queried function's for outgoing calls.
    pub(crate) fn call_edge(&self, raw: &RawCall, sites_in: &Path) -> CallEdge {
        CallEdge {
            item: self.call_item(&raw.item),
            sites: raw
                .ranges
                .iter()
                .map(|range| self.location_at(sites_in, *range))
                .collect(),
        }
    }

    pub(crate) fn diagnostic(&self, path: &Path, raw: &RawDiagnostic) -> Diagnostic {
        let (line, column, end_line, end_column) = self.span(path, raw.range);
        Diagnostic {
            path: path.to_path_buf(),
            line,
            column,
            end_line,
            end_column,
            severity: Severity::from_lsp(raw.severity),
            message: raw.message.clone(),
            code: raw.code.clone(),
            source: raw.source.clone(),
        }
    }

    pub(crate) fn edit_plan(
        &self,
        raw: &BTreeMap<PathBuf, Vec<(LspRange, String)>>,
    ) -> WorkspaceEditPlan {
        let files = raw
            .iter()
            .map(|(path, edits)| {
                let mut edits: Vec<TextEditPlan> = edits
                    .iter()
                    .map(|(range, new_text)| {
                        let (start_line, start_col, end_line, end_col) = self.span(path, *range);
                        TextEditPlan {
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            new_text: new_text.clone(),
                        }
                    })
                    .collect();
                edits.sort_by_key(|edit| (edit.start_line, edit.start_col));
                FileEdits {
                    path: path.clone(),
                    edits,
                }
            })
            .collect();
        WorkspaceEditPlan { files }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(line: u32, start: u32, end: u32) -> LspRange {
        LspRange {
            start: LspPosition {
                line,
                character: start,
            },
            end: LspPosition {
                line,
                character: end,
            },
        }
    }

    #[tokio::test]
    async fn converts_utf16_ranges_with_the_synced_text() {
        let documents = Documents::default();
        let path = Path::new("/virtual/a.rs");
        documents.update(path, "// 🦀 crab\r\nlet x = \"é🦀\"; call(x);\n");
        let mut texts = Texts::new(&documents);
        texts.load(path).await;
        // `call` is character 14 but UTF-16 offset 15 (the crab counts twice).
        let location = texts.location(&RawLocation {
            path: path.to_path_buf(),
            range: range(1, 15, 19),
        });
        assert_eq!(
            (location.line, location.column, location.end_column),
            (2, 15, 19)
        );
        assert_eq!(
            location.snippet.as_deref(),
            Some("let x = \"é🦀\"; call(x);")
        );
        let first = texts.location(&RawLocation {
            path: path.to_path_buf(),
            range: range(0, 6, 10),
        });
        assert_eq!(first.column, 6, "after `// 🦀 `");
    }

    #[tokio::test]
    async fn unreadable_files_keep_raw_offsets() {
        let documents = Documents::default();
        let path = Path::new("/definitely/missing/file.rs");
        let mut texts = Texts::new(&documents);
        texts.load(path).await;
        let location = texts.location(&RawLocation {
            path: path.to_path_buf(),
            range: range(4, 7, 9),
        });
        assert_eq!((location.line, location.column), (5, 8));
        assert!(location.snippet.is_none());
    }
}
