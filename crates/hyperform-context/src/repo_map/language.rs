//! Languages the repo map can outline, chosen by file extension.

use std::path::Path;

use tree_sitter::Parser;

use super::symbol::{Outliner, Symbol};
use super::{ecmascript, go, python, rust};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lang {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
}

impl Lang {
    /// Case-insensitive; accepts `rs` or `.rs`.
    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.strip_prefix('.').unwrap_or(ext).to_ascii_lowercase();
        Some(match ext.as_str() {
            "rs" => Self::Rust,
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "py" => Self::Python,
            "go" => Self::Go,
            _ => return None,
        })
    }

    pub fn from_path(path: &str) -> Option<Self> {
        Self::from_extension(Path::new(path).extension()?.to_str()?)
    }

    /// The file's definitions in source order, or `None` when the grammar
    /// cannot be loaded or tree-sitter returns no tree.
    pub fn outline(self, parser: &mut Parser, text: &str) -> Option<Vec<Symbol>> {
        parser.set_language(&self.grammar()).ok()?;
        let tree = parser.parse(text, None)?;
        let mut outliner = Outliner::new(text);
        let root = tree.root_node();
        match self {
            Self::Rust => rust::outline(&mut outliner, root),
            Self::TypeScript | Self::Tsx | Self::JavaScript => {
                ecmascript::outline(&mut outliner, root);
            }
            Self::Python => python::outline(&mut outliner, root),
            Self::Go => go::outline(&mut outliner, root),
        }
        Some(outliner.symbols)
    }

    fn grammar(self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }
}

/// Whether files with extension `ext` (`rs` or `.rs`) appear in the repo map.
pub fn supported_extension(ext: &str) -> bool {
    Lang::from_extension(ext).is_some()
}

/// Test helper: outline `text` as the language of `path`.
#[cfg(test)]
pub(crate) fn outline_for_test(path: &str, text: &str) -> Vec<Symbol> {
    let lang = Lang::from_path(path).expect("supported path");
    lang.outline(&mut Parser::new(), text)
        .expect("grammar loads")
}

/// Test helper: `kind name` at `depth` for every symbol, as display lines.
#[cfg(test)]
pub(crate) fn describe(symbols: &[Symbol]) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| {
            format!(
                "{}{} {} L{}",
                "  ".repeat(symbol.depth),
                symbol.kind,
                symbol.name,
                symbol.line
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_map_to_languages() {
        for ext in [
            "rs", ".RS", "ts", "mts", "tsx", "js", "jsx", "mjs", "cjs", "py", "go",
        ] {
            assert!(supported_extension(ext), "{ext}");
        }
        for ext in ["", "md", "c", "pyi", "json", "rs.bak"] {
            assert!(!supported_extension(ext), "{ext}");
        }
        assert_eq!(Lang::from_path("ui/App.tsx"), Some(Lang::Tsx));
        assert_eq!(Lang::from_path("dir.rs/README"), None);
        assert_eq!(Lang::from_path("Makefile"), None);
    }

    #[test]
    fn every_grammar_loads() {
        let mut parser = Parser::new();
        for lang in [
            Lang::Rust,
            Lang::TypeScript,
            Lang::Tsx,
            Lang::JavaScript,
            Lang::Python,
            Lang::Go,
        ] {
            assert!(lang.outline(&mut parser, "").is_some(), "{lang:?}");
        }
    }
}
