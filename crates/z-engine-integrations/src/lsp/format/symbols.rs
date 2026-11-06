//! A file's symbol tree (kinds, names, details, line ranges) and hover text.

use super::super::types::{Hover, SymbolNode};

/// One line per symbol, children indented by two spaces.
pub fn format_symbols(symbols: &[SymbolNode]) -> String {
    if symbols.is_empty() {
        return "No symbols found.".to_string();
    }
    let mut lines = Vec::new();
    push_symbols(symbols, 0, &mut lines);
    lines.join("\n")
}

fn push_symbols(symbols: &[SymbolNode], depth: usize, lines: &mut Vec<String>) {
    for symbol in symbols {
        let detail = symbol
            .detail
            .as_deref()
            .map(|detail| format!(": {}", detail.replace('\n', " ")))
            .unwrap_or_default();
        let span = if symbol.start_line == symbol.end_line {
            format!("line {}", symbol.start_line)
        } else {
            format!("lines {}-{}", symbol.start_line, symbol.end_line)
        };
        lines.push(format!(
            "{}{} {}{detail} ({span})",
            "  ".repeat(depth),
            symbol.kind.label(),
            symbol.name
        ));
        push_symbols(&symbol.children, depth + 1, lines);
    }
}

/// The hover markdown, or a note that there is none.
pub fn format_hover(hover: Option<&Hover>) -> String {
    match hover {
        Some(hover) => hover.contents.trim().to_string(),
        None => "No hover information at this position.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::types::SymbolKind;

    fn node(name: &str, kind: u32, lines: (u32, u32), children: Vec<SymbolNode>) -> SymbolNode {
        SymbolNode {
            name: name.into(),
            detail: None,
            kind: SymbolKind(kind),
            line: lines.0,
            column: 1,
            start_line: lines.0,
            end_line: lines.1,
            children,
        }
    }

    #[test]
    fn renders_a_tree_with_ranges() {
        let mut function = node("greet", 12, (5, 7), Vec::new());
        function.detail = Some("fn(g: &Greeter)\n-> String".into());
        let tree = vec![
            node(
                "Greeter",
                23,
                (1, 3),
                vec![node("name", 8, (2, 2), Vec::new())],
            ),
            function,
        ];
        assert_eq!(
            format_symbols(&tree),
            "struct Greeter (lines 1-3)\n  field name (line 2)\nfunction greet: fn(g: &Greeter) -> String (lines 5-7)"
        );
        assert_eq!(format_symbols(&[]), "No symbols found.");
    }

    #[test]
    fn hover_passes_markdown_through() {
        let hover = Hover {
            contents: "```rust\nfn greet()\n```\n".into(),
        };
        assert_eq!(format_hover(Some(&hover)), "```rust\nfn greet()\n```");
        assert_eq!(format_hover(None), "No hover information at this position.");
    }
}
