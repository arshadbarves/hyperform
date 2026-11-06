//! Python outlines: functions, classes with their methods and nested
//! classes (decorated or not), module-level `UPPER_CASE` constants and
//! `type` aliases. Function bodies are not searched.

use tree_sitter::Node;

use super::symbol::{MAX_DEPTH, Outliner, identifier, named_children};

pub(crate) fn outline(out: &mut Outliner<'_>, root: Node<'_>) {
    block(out, root, 0);
}

fn block(out: &mut Outliner<'_>, parent: Node<'_>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for node in named_children(parent) {
        statement(out, node, depth);
    }
}

fn statement(out: &mut Outliner<'_>, node: Node<'_>, depth: usize) {
    match node.kind() {
        "function_definition" => {
            out.named("def", node, depth);
        }
        "class_definition" => {
            out.named_with_members("class", node, depth, |out, depth| {
                if let Some(body) = node.child_by_field_name("body") {
                    block(out, body, depth);
                }
            });
        }
        "decorated_definition" => {
            if let Some(definition) = node.child_by_field_name("definition") {
                statement(out, definition, depth);
            }
        }
        "expression_statement" if depth == 0 => constants(out, node),
        "type_alias_statement" if depth == 0 => {
            if let Some(name) = out.field_text(node, "left") {
                let ident = name.split('[').next().and_then(identifier);
                out.push("type", node, name, ident, depth);
            }
        }
        _ => {}
    }
}

fn constants(out: &mut Outliner<'_>, statement: Node<'_>) {
    for assignment in named_children(statement) {
        if assignment.kind() != "assignment" {
            continue;
        }
        let name = assignment
            .child_by_field_name("left")
            .filter(|left| left.kind() == "identifier")
            .and_then(|left| out.text(left));
        if let Some(name) = name.filter(|name| is_constant_name(name)) {
            out.push("const", assignment, name, Some(name), 0);
        }
    }
}

/// `MAX_RETRIES`, `_DEFAULT_TIMEOUT`: upper case by convention.
fn is_constant_name(name: &str) -> bool {
    name.chars().any(|c| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::super::language::{describe, outline_for_test};

    const SAMPLE: &str = r#"import os

MAX_RETRIES = 3
logger = make_logger()
__all__ = ["Client"]

type UserId = int

@dataclass
class Client:
    TIMEOUT = 5

    def __init__(self, url):
        def local():
            pass
        self.url = url

    @property
    def host(self):
        return self.url

    class Error(Exception):
        pass

async def fetch(client):
    return None
"#;

    #[test]
    fn outlines_classes_methods_and_constants() {
        let symbols = outline_for_test("pkg/client.py", SAMPLE);
        assert_eq!(
            describe(&symbols),
            [
                "const MAX_RETRIES L3",
                "type UserId L7",
                "class Client L10",
                "  def __init__ L13",
                "  def host L19",
                "  class Error L22",
                "def fetch L25",
            ]
        );
    }
}
