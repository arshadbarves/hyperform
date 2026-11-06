//! Rust outlines: items, impl blocks with their members, trait members and
//! inline modules. Function bodies are not searched. Test-only code
//! (`#[test]` functions, the contents of `#[cfg(test)]` modules) is left out
//! so tests do not crowd the map.

use tree_sitter::Node;

use super::symbol::{MAX_DEPTH, Outliner, named_children};

pub(crate) fn outline(out: &mut Outliner<'_>, root: Node<'_>) {
    items(out, root, 0);
}

fn items(out: &mut Outliner<'_>, parent: Node<'_>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for node in named_children(parent) {
        item(out, node, depth);
    }
}

fn item(out: &mut Outliner<'_>, node: Node<'_>, depth: usize) {
    let kind = match node.kind() {
        "function_item" | "function_signature_item" => "fn",
        "struct_item" => "struct",
        "enum_item" => "enum",
        "union_item" => "union",
        "type_item" | "associated_type" => "type",
        "const_item" => "const",
        "static_item" => "static",
        "macro_definition" => "macro",
        "trait_item" => "trait",
        "mod_item" => "mod",
        "impl_item" => {
            if impl_block(out, node, depth) {
                body(out, node, depth + 1);
            }
            return;
        }
        _ => return,
    };
    let test_only = is_test_only(out, node);
    if kind == "fn" && test_only {
        return;
    }
    let recorded = out.named(kind, node, depth);
    if recorded && matches!(kind, "trait" | "mod") && !test_only {
        body(out, node, depth + 1);
    }
}

fn body(out: &mut Outliner<'_>, node: Node<'_>, depth: usize) {
    if let Some(body) = node.child_by_field_name("body") {
        items(out, body, depth);
    }
}

/// Records `impl Type` or `impl Trait for Type`.
fn impl_block(out: &mut Outliner<'_>, node: Node<'_>, depth: usize) -> bool {
    let Some(target) = out.field_text(node, "type") else {
        return false;
    };
    let name = match out.field_text(node, "trait") {
        Some(trait_name) => format!("{trait_name} for {target}"),
        None => target.to_string(),
    };
    out.push("impl", node, &name, None, depth);
    true
}

/// True when an attribute directly above `node` marks it as test code.
fn is_test_only(out: &Outliner<'_>, node: Node<'_>) -> bool {
    let mut previous = node.prev_named_sibling();
    while let Some(sibling) = previous {
        match sibling.kind() {
            "attribute_item" => {
                if out.text(sibling).is_some_and(is_test_attribute) {
                    return true;
                }
            }
            "line_comment" | "block_comment" => {}
            _ => return false,
        }
        previous = sibling.prev_named_sibling();
    }
    false
}

/// `#[test]`, `#[tokio::test(...)]`, `#[cfg(test)]` and similar.
fn is_test_attribute(attribute: &str) -> bool {
    let compact: String = attribute.chars().filter(|c| !c.is_whitespace()).collect();
    compact == "#[test]"
        || compact.starts_with("#[cfg(test)")
        || compact.ends_with("::test]")
        || compact.contains("::test(")
}

#[cfg(test)]
mod tests {
    use super::super::language::{describe, outline_for_test};

    const SAMPLE: &str = r#"use std::fmt;

/// A striped animal.
#[derive(Debug)]
pub struct Zebra {
    pub stripes: u32,
}

pub enum Mood { Happy, Grumpy }

pub trait Runner {
    type Output;
    fn run(&self) -> Self::Output;
    fn rest(&self) {}
}

impl<T> fmt::Display for Wrapper<T> where T: fmt::Debug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { Ok(()) }
}

impl Zebra {
    pub const LEGS: u8 = 4;
    pub fn new() -> Self { fn hidden_helper() {} Self { stripes: 1 } }
}

pub type Herd = Vec<Zebra>;
static COUNT: u32 = 0;
const MAX_STRIPES: u32 = 99;
macro_rules! stripe { () => {} }

pub mod outer {
    pub mod inner {
        pub fn deep() {}
    }
}

#[test]
fn top_level_test() {}

#[cfg(test)]
mod tests {
    #[test]
    fn hidden() {}
}
"#;

    #[test]
    fn outlines_items_members_and_nesting() {
        let symbols = outline_for_test("src/lib.rs", SAMPLE);
        assert_eq!(
            describe(&symbols),
            [
                "struct Zebra L5",
                "enum Mood L9",
                "trait Runner L11",
                "  type Output L12",
                "  fn run L13",
                "  fn rest L14",
                "impl fmt::Display for Wrapper<T> L17",
                "  fn fmt L18",
                "impl Zebra L21",
                "  const LEGS L22",
                "  fn new L23",
                "type Herd L26",
                "static COUNT L27",
                "const MAX_STRIPES L28",
                "macro stripe L29",
                "mod outer L31",
                "  mod inner L32",
                "    fn deep L33",
                "mod tests L41",
            ]
        );
        let impl_block = symbols.iter().find(|s| s.kind == "impl").unwrap();
        assert_eq!(impl_block.ident, None);
        let new = symbols.iter().find(|s| s.name == "new").unwrap();
        assert_eq!(new.ident.as_deref(), Some("new"));
    }

    #[test]
    fn tokio_tests_are_skipped() {
        let symbols = outline_for_test(
            "tests/flow.rs",
            "#[tokio::test(flavor = \"multi_thread\")]\nasync fn flow() {}\nfn helper() {}\n",
        );
        assert_eq!(describe(&symbols), ["fn helper L3"]);
    }
}
