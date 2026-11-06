//! Go outlines: functions, methods (shown as `Receiver.Name`), struct,
//! interface and other type declarations with interface methods, type
//! aliases, and package-level `const`s and `var`s.

use tree_sitter::Node;

use super::symbol::{Outliner, named_children};

pub(crate) fn outline(out: &mut Outliner<'_>, root: Node<'_>) {
    for node in named_children(root) {
        match node.kind() {
            "function_declaration" => {
                out.named("func", node, 0);
            }
            "method_declaration" => method(out, node),
            "type_declaration" => types(out, node),
            "const_declaration" => specs(out, node, "const"),
            "var_declaration" => specs(out, node, "var"),
            _ => {}
        }
    }
}

fn method(out: &mut Outliner<'_>, node: Node<'_>) {
    let Some(name) = out.field_text(node, "name") else {
        return;
    };
    let display = match receiver_type(out, node) {
        Some(receiver) => format!("{receiver}.{name}"),
        None => name.to_string(),
    };
    out.push("method", node, &display, Some(name), 0);
}

/// `Server` for receivers `(s Server)`, `(s *Server)` and `(s *Server[T])`.
fn receiver_type<'s>(out: &Outliner<'s>, method: Node<'_>) -> Option<&'s str> {
    let receiver = method.child_by_field_name("receiver")?;
    let parameter = named_children(receiver)
        .into_iter()
        .find(|node| node.kind() == "parameter_declaration")?;
    let mut ty = parameter.child_by_field_name("type")?;
    loop {
        ty = match ty.kind() {
            "pointer_type" => named_children(ty).into_iter().next()?,
            "generic_type" => ty.child_by_field_name("type")?,
            _ => return out.text(ty),
        };
    }
}

fn types(out: &mut Outliner<'_>, declaration: Node<'_>) {
    for spec in named_children(declaration) {
        match spec.kind() {
            "type_spec" => {
                let ty = spec.child_by_field_name("type");
                let kind = match ty.map(|ty| ty.kind()) {
                    Some("struct_type") => "struct",
                    Some("interface_type") => "interface",
                    _ => "type",
                };
                if !out.named(kind, spec, 0) || kind != "interface" {
                    continue;
                }
                for member in ty.map(named_children).unwrap_or_default() {
                    if member.kind() == "method_elem" {
                        out.named("method", member, 1);
                    }
                }
            }
            "type_alias" => {
                out.named("type", spec, 0);
            }
            _ => {}
        }
    }
}

/// Every name in `const (...)` / `var (...)` groups, one symbol each.
fn specs(out: &mut Outliner<'_>, declaration: Node<'_>, kind: &'static str) {
    for spec in named_children(declaration) {
        match spec.kind() {
            "var_spec_list" => specs(out, spec, kind),
            "const_spec" | "var_spec" => {
                let mut cursor = spec.walk();
                let names: Vec<Node<'_>> =
                    spec.children_by_field_name("name", &mut cursor).collect();
                for name_node in names.into_iter().filter(|n| n.kind() == "identifier") {
                    if let Some(name) = out.text(name_node).filter(|name| *name != "_") {
                        out.push(kind, name_node, name, Some(name), 0);
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::language::{describe, outline_for_test};

    const SAMPLE: &str = r#"package server

import "net/http"

type Server[T any] struct {
	addr string
}

type Handler interface {
	Serve(w http.ResponseWriter) error
	Close()
}

type Port = int
type Mode int

const (
	ModeDev Mode = iota
	ModeProd
)

var ErrClosed, _ = errors.New("closed"), 0

func New(addr string) *Server[int] { return nil }

func (s *Server[T]) Start() error { return nil }

func (h handler) Close() {}
"#;

    #[test]
    fn outlines_types_methods_and_package_values() {
        let symbols = outline_for_test("server/server.go", SAMPLE);
        assert_eq!(
            describe(&symbols),
            [
                "struct Server L5",
                "interface Handler L9",
                "  method Serve L10",
                "  method Close L11",
                "type Port L14",
                "type Mode L15",
                "const ModeDev L18",
                "const ModeProd L19",
                "var ErrClosed L22",
                "func New L24",
                "method Server.Start L26",
                "method handler.Close L28",
            ]
        );
        let start = symbols.iter().find(|s| s.name == "Server.Start").unwrap();
        assert_eq!(start.ident.as_deref(), Some("Start"));
    }
}
