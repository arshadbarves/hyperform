//! Definitions extracted from one file, and the collector the language
//! walkers share.

use tree_sitter::Node;

use crate::text::single_line;

/// Deepest nesting the walkers descend into (e.g. mod > impl > fn is 2).
pub(crate) const MAX_DEPTH: usize = 4;
/// Longer display names (usually generic impl targets) are clipped.
const MAX_NAME_CHARS: usize = 80;

/// One definition in a file outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Symbol {
    pub kind: &'static str,
    /// Display name, e.g. `parse`, `fmt::Display for Config`, `Server.Start`.
    pub name: String,
    /// Identifier other files use to refer to this definition; `None` when
    /// the definition introduces no name (impl blocks, string modules).
    pub ident: Option<String>,
    /// 1-based line where the definition starts.
    pub line: usize,
    /// 0 for top-level definitions, 1 for their members, and so on.
    pub depth: usize,
}

/// Collects symbols in source order while a walker visits a syntax tree.
#[derive(Debug)]
pub(crate) struct Outliner<'s> {
    src: &'s [u8],
    pub symbols: Vec<Symbol>,
}

impl<'s> Outliner<'s> {
    pub fn new(src: &'s str) -> Self {
        Self {
            src: src.as_bytes(),
            symbols: Vec::new(),
        }
    }

    pub fn text(&self, node: Node<'_>) -> Option<&'s str> {
        node.utf8_text(self.src).ok()
    }

    pub fn field_text(&self, node: Node<'_>, field: &str) -> Option<&'s str> {
        self.text(node.child_by_field_name(field)?)
    }

    /// Records `node` under the text of its `name` field; false when the
    /// node has no name.
    pub fn named(&mut self, kind: &'static str, node: Node<'_>, depth: usize) -> bool {
        match self.field_text(node, "name") {
            Some(name) => {
                self.push(kind, node, name, identifier(name), depth);
                true
            }
            None => false,
        }
    }

    /// Like [`Self::named`], then visits the definition's members one level
    /// deeper; anonymous containers are skipped with their members.
    pub fn named_with_members(
        &mut self,
        kind: &'static str,
        node: Node<'_>,
        depth: usize,
        members: impl FnOnce(&mut Self, usize),
    ) {
        if self.named(kind, node, depth) {
            members(self, depth + 1);
        }
    }

    /// Records a definition starting at `node` with an explicit name.
    pub fn push(
        &mut self,
        kind: &'static str,
        node: Node<'_>,
        name: &str,
        ident: Option<&str>,
        depth: usize,
    ) {
        self.symbols.push(Symbol {
            kind,
            name: clip(&single_line(name)),
            ident: ident.map(str::to_string),
            line: node.start_position().row + 1,
            depth,
        });
    }
}

/// `name` when it is a plain identifier other files could mention.
pub(crate) fn identifier(name: &str) -> Option<&str> {
    let plain = !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    plain.then_some(name)
}

/// Named children, collected so walkers can recurse while iterating.
pub(crate) fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

fn clip(name: &str) -> String {
    match name.char_indices().nth(MAX_NAME_CHARS) {
        Some((end, _)) => format!("{}…", &name[..end]),
        None => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_exclude_strings_paths_and_numbers() {
        assert_eq!(identifier("parse_all"), Some("parse_all"));
        assert_eq!(identifier("Größe"), Some("Größe"));
        assert_eq!(identifier("\"left-pad\""), None);
        assert_eq!(identifier("Display for Foo"), None);
        assert_eq!(identifier("9lives"), None);
        assert_eq!(identifier(""), None);
    }

    #[test]
    fn long_names_are_clipped() {
        let long = "T".repeat(100);
        let clipped = clip(&long);
        assert_eq!(clipped.chars().count(), MAX_NAME_CHARS + 1);
        assert!(clipped.ends_with('…'));
        assert_eq!(clip("short"), "short");
    }
}
