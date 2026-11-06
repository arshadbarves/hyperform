//! TypeScript, TSX and JavaScript outlines: functions, classes with their
//! methods, interfaces, type aliases, enums, namespaces, `const`s and
//! function-valued variables, including exported and `declare`d forms and
//! CommonJS `exports.name = function` assignments. Function bodies are not
//! searched.

use tree_sitter::Node;

use super::symbol::{MAX_DEPTH, Outliner, identifier, named_children};

pub(crate) fn outline(out: &mut Outliner<'_>, root: Node<'_>) {
    statements(out, root, 0);
}

fn statements(out: &mut Outliner<'_>, parent: Node<'_>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for node in named_children(parent) {
        statement(out, node, depth);
    }
}

fn statement(out: &mut Outliner<'_>, node: Node<'_>, depth: usize) {
    match node.kind() {
        "export_statement" => {
            if let Some(declaration) = node.child_by_field_name("declaration") {
                statement(out, declaration, depth);
            }
        }
        "ambient_declaration" => statements(out, node, depth),
        "expression_statement" => expression(out, node, depth),
        "function_declaration" | "generator_function_declaration" | "function_signature" => {
            out.named("function", node, depth);
        }
        "class_declaration" | "abstract_class_declaration" => {
            out.named_with_members("class", node, depth, |out, depth| {
                class_members(out, node, depth);
            });
        }
        "interface_declaration" => {
            out.named_with_members("interface", node, depth, |out, depth| {
                interface_members(out, node, depth);
            });
        }
        "type_alias_declaration" => {
            out.named("type", node, depth);
        }
        "enum_declaration" => {
            out.named("enum", node, depth);
        }
        "internal_module" | "module" => {
            let kind = if node.kind() == "module" {
                "module"
            } else {
                "namespace"
            };
            out.named_with_members(kind, node, depth, |out, depth| {
                if let Some(body) = node.child_by_field_name("body") {
                    statements(out, body, depth);
                }
            });
        }
        "lexical_declaration" | "variable_declaration" => declarators(out, node, depth),
        _ => {}
    }
}

/// `namespace X {}` parses as an expression; CommonJS exports are
/// assignments of functions.
fn expression(out: &mut Outliner<'_>, node: Node<'_>, depth: usize) {
    for child in named_children(node) {
        match child.kind() {
            "internal_module" => statement(out, child, depth),
            "assignment_expression" => assignment(out, child, depth),
            _ => {}
        }
    }
}

fn assignment(out: &mut Outliner<'_>, node: Node<'_>, depth: usize) {
    let (Some(left), Some(right)) = (
        node.child_by_field_name("left"),
        node.child_by_field_name("right"),
    ) else {
        return;
    };
    if !is_function(right) {
        return;
    }
    let target = match left.kind() {
        "identifier" => Some(left),
        "member_expression" => left.child_by_field_name("property"),
        _ => None,
    };
    // `module.exports = function name() {}` is named by the function.
    let name = target
        .and_then(|target| out.text(target))
        .filter(|name| *name != "exports")
        .or_else(|| out.field_text(right, "name"));
    if let Some(name) = name {
        out.push("function", node, name, identifier(name), depth);
    }
}

/// Function- or class-valued variables at any declaration level, plus
/// plain `const`s.
fn declarators(out: &mut Outliner<'_>, node: Node<'_>, depth: usize) {
    let is_const = node
        .child_by_field_name("kind")
        .is_some_and(|kind| kind.kind() == "const");
    for declarator in named_children(node) {
        if declarator.kind() != "variable_declarator" {
            continue;
        }
        let Some(name) = declarator
            .child_by_field_name("name")
            .filter(|name| name.kind() == "identifier")
            .and_then(|name| out.text(name))
        else {
            continue;
        };
        let value = declarator.child_by_field_name("value");
        let kind = match value {
            Some(value) if is_function(value) => "function",
            Some(value) if value.kind() == "class" => "class",
            Some(value) if is_require(out, value) => continue,
            _ if is_const => "const",
            _ => continue,
        };
        out.push(kind, declarator, name, identifier(name), depth);
    }
}

fn class_members(out: &mut Outliner<'_>, class: Node<'_>, depth: usize) {
    let Some(body) = class.child_by_field_name("body") else {
        return;
    };
    for member in named_children(body) {
        match member.kind() {
            "method_definition" | "method_signature" | "abstract_method_signature" => {
                out.named("method", member, depth);
            }
            // Arrow-function class fields (`onClick = () => {}`) act as methods.
            "public_field_definition" | "field_definition" => {
                if !member.child_by_field_name("value").is_some_and(is_function) {
                    continue;
                }
                let name = member
                    .child_by_field_name("name")
                    .or_else(|| member.child_by_field_name("property"))
                    .and_then(|name| out.text(name));
                if let Some(name) = name {
                    out.push("method", member, name, identifier(name), depth);
                }
            }
            _ => {}
        }
    }
}

fn interface_members(out: &mut Outliner<'_>, interface: Node<'_>, depth: usize) {
    let Some(body) = interface.child_by_field_name("body") else {
        return;
    };
    for member in named_children(body) {
        if member.kind() == "method_signature" {
            out.named("method", member, depth);
        }
    }
}

/// `require("x")` is an import, not a definition.
fn is_require(out: &Outliner<'_>, node: Node<'_>) -> bool {
    node.kind() == "call_expression" && out.field_text(node, "function") == Some("require")
}

fn is_function(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "arrow_function" | "function_expression" | "function" | "generator_function"
    )
}

#[cfg(test)]
#[path = "ecmascript_tests.rs"]
mod tests;
