//! Locations, workspace symbols and call hierarchies as `path:line:col`
//! lines relative to the project root, with the source line when known.

use std::path::Path;

use z_engine_host::relative_display;

use super::super::types::{CallDirection, CallEdge, Location, WorkspaceSymbol};
use super::{MAX_ITEMS, more};

/// `path:line:col` relative to `root`.
pub(crate) fn position(root: &Path, path: &Path, line: u32, column: u32) -> String {
    format!("{}:{line}:{column}", relative_display(root, path))
}

fn location_line(root: &Path, location: &Location) -> String {
    let at = position(root, &location.path, location.line, location.column);
    match location.snippet.as_deref() {
        Some(snippet) if !snippet.is_empty() => format!("{at}: {snippet}"),
        _ => at,
    }
}

/// One `path:line:col: source` line per location.
pub fn format_locations(root: &Path, locations: &[Location]) -> String {
    if locations.is_empty() {
        return "No locations found.".to_string();
    }
    let mut lines: Vec<String> = locations
        .iter()
        .take(MAX_ITEMS)
        .map(|location| location_line(root, location))
        .collect();
    lines.extend(more(locations.len()));
    lines.join("\n")
}

/// `kind name (in container) - path:line:col`.
pub fn format_workspace_symbols(root: &Path, symbols: &[WorkspaceSymbol]) -> String {
    if symbols.is_empty() {
        return "No matching symbols.".to_string();
    }
    let mut lines: Vec<String> = symbols
        .iter()
        .take(MAX_ITEMS)
        .map(|symbol| {
            let container = symbol
                .container
                .as_deref()
                .map(|c| format!(" in {c}"))
                .unwrap_or_default();
            let at = position(
                root,
                &symbol.location.path,
                symbol.location.line,
                symbol.location.column,
            );
            format!("{} {}{container} - {at}", symbol.kind.label(), symbol.name)
        })
        .collect();
    lines.extend(more(symbols.len()));
    lines.join("\n")
}

/// Each caller (or callee) with its call sites indented below it.
pub fn format_calls(root: &Path, edges: &[CallEdge], direction: CallDirection) -> String {
    if edges.is_empty() {
        return match direction {
            CallDirection::Incoming => "No callers found.",
            CallDirection::Outgoing => "No calls found.",
        }
        .to_string();
    }
    let mut lines = Vec::new();
    for edge in edges.iter().take(MAX_ITEMS) {
        let item = &edge.item;
        let at = position(
            root,
            &item.location.path,
            item.location.line,
            item.location.column,
        );
        lines.push(format!("{} {} - {at}", item.kind.label(), item.name));
        lines.extend(
            edge.sites
                .iter()
                .map(|site| format!("  called at {}", location_line(root, site))),
        );
    }
    lines.extend(more(edges.len()));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::lsp::types::{CallItem, SymbolKind};

    fn location(path: &str, line: u32, column: u32, snippet: Option<&str>) -> Location {
        Location {
            path: PathBuf::from(path),
            line,
            column,
            end_line: line,
            end_column: column + 3,
            snippet: snippet.map(str::to_string),
        }
    }

    #[test]
    fn locations_are_relative_with_snippets() {
        let root = Path::new("/proj");
        let text = format_locations(
            root,
            &[
                location("/proj/src/lib.rs", 3, 8, Some("fn helper() {}")),
                location("/elsewhere/x.rs", 1, 1, None),
            ],
        );
        assert_eq!(text, "src/lib.rs:3:8: fn helper() {}\n/elsewhere/x.rs:1:1");
        assert_eq!(format_locations(root, &[]), "No locations found.");
        let many: Vec<_> = (0..MAX_ITEMS as u32 + 5)
            .map(|i| location("/proj/a.rs", i + 1, 1, None))
            .collect();
        assert!(format_locations(root, &many).ends_with("... and 5 more"));
    }

    #[test]
    fn symbols_and_calls_render_kinds() {
        let root = Path::new("/proj");
        let symbol = WorkspaceSymbol {
            name: "Greeter".into(),
            kind: SymbolKind(23),
            container: Some("demo".into()),
            location: location("/proj/src/lib.rs", 1, 8, None),
        };
        assert_eq!(
            format_workspace_symbols(root, &[symbol]),
            "struct Greeter in demo - src/lib.rs:1:8"
        );
        let edge = CallEdge {
            item: CallItem {
                name: "main".into(),
                kind: SymbolKind(12),
                detail: None,
                location: location("/proj/src/main.rs", 4, 4, None),
            },
            sites: vec![location("/proj/src/main.rs", 6, 5, Some("greet(&g);"))],
        };
        assert_eq!(
            format_calls(root, &[edge], CallDirection::Incoming),
            "function main - src/main.rs:4:4\n  called at src/main.rs:6:5: greet(&g);"
        );
        assert_eq!(
            format_calls(root, &[], CallDirection::Outgoing),
            "No calls found."
        );
    }
}
