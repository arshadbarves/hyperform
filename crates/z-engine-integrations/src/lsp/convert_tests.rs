use std::path::Path;

use serde_json::json;

use super::*;

fn range(line: u32, start: u32, end: u32) -> Value {
    json!({"start": {"line": line, "character": start}, "end": {"line": line, "character": end}})
}

#[test]
fn locations_accept_every_shape() {
    assert!(locations(&Value::Null).unwrap().is_empty());
    let single = locations(&json!({"uri": "file:///a.rs", "range": range(1, 2, 3)})).unwrap();
    assert_eq!(single[0].path, Path::new("/a.rs"));
    assert_eq!(single[0].range.start.character, 2);
    let links = locations(&json!([{
        "targetUri": "file:///b.rs", "targetRange": range(0, 0, 9), "targetSelectionRange": range(0, 3, 5)
    }, {"uri": "jdt://contents/x", "range": range(0, 0, 1)}]))
    .unwrap();
    assert_eq!(links.len(), 1, "non-file URIs are skipped");
    assert_eq!(links[0].range.start.character, 3);
    assert!(locations(&json!([{"uri": "file:///a.rs"}])).is_err());
    assert!(locations(&json!("nope")).is_err());
}

#[test]
fn symbols_accept_trees_and_flat_lists() {
    let tree = document_symbols(&json!([{
        "name": "S", "kind": 23, "range": range(0, 0, 9), "selectionRange": range(0, 7, 8),
        "children": [{"name": "f", "kind": 8, "range": range(1, 4, 9), "selectionRange": range(1, 4, 5)}]
    }]))
    .unwrap();
    assert_eq!(tree[0].children[0].name, "f");
    let flat = document_symbols(&json!([{
        "name": "g", "kind": 12, "location": {"uri": "file:///a.rs", "range": range(4, 3, 4)}, "containerName": "m"
    }]))
    .unwrap();
    assert_eq!(flat[0].selection_range.start.line, 4);
    assert_eq!(flat[0].detail.as_deref(), Some("m"));
    let workspace = workspace_symbols(&json!([
        {"name": "g", "kind": 12, "location": {"uri": "file:///a.rs", "range": range(4, 3, 4)}},
        {"name": "h", "kind": 12, "location": {"uri": "file:///b.rs"}}
    ]))
    .unwrap();
    assert_eq!(workspace.len(), 2);
    assert_eq!(workspace[1].location.range.start.line, 0);
}

#[test]
fn call_hierarchy_keeps_the_wire_item() {
    let item = json!({"name": "f", "kind": 12, "uri": "file:///a.rs", "range": range(2, 0, 9),
                      "selectionRange": range(2, 3, 4), "data": {"id": 7}});
    let items = call_items(&json!([item.clone()])).unwrap();
    assert_eq!(items[0].wire, item);
    let incoming = calls(
        &json!([{"from": item, "fromRanges": [range(5, 4, 5)]}]),
        "from",
    )
    .unwrap();
    assert_eq!(incoming[0].ranges[0].start.line, 5);
    assert!(calls(&json!([{"to": {}}]), "from").is_err());
}

#[test]
fn workspace_edits_merge_per_file_and_refuse_file_operations() {
    let changes = workspace_edit(&json!({"changes": {
        "file:///a.rs": [{"range": range(0, 3, 6), "newText": "bar"}],
        "file:///b.rs": [{"range": range(1, 0, 3), "newText": "bar"}]
    }}))
    .unwrap();
    assert_eq!(changes.len(), 2);
    let documents = workspace_edit(&json!({"documentChanges": [
        {"textDocument": {"uri": "file:///a.rs", "version": 1}, "edits": [{"range": range(0, 0, 1), "newText": "x"}]},
        {"textDocument": {"uri": "file:///a.rs", "version": 1}, "edits": [{"range": range(2, 0, 1), "newText": "y"}]}
    ]}))
    .unwrap();
    assert_eq!(documents[Path::new("/a.rs")].len(), 2);
    let rename = json!({"documentChanges": [{"kind": "rename", "oldUri": "file:///a", "newUri": "file:///b"}]});
    assert!(matches!(
        workspace_edit(&rename),
        Err(IntegrationError::Unsupported(_))
    ));
    assert!(workspace_edit(&json!({"oops": 1})).is_err());
}

#[test]
fn diagnostics_and_hover_decode() {
    let items = vec![
        json!({"range": range(3, 1, 2), "severity": 2, "code": 42, "source": "x", "message": "m"}),
        json!({"range": range(3, 1, 2), "code": "E1", "message": "n"}),
        json!({"message": "no range"}),
    ];
    let decoded = diagnostics(&items);
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].code.as_deref(), Some("42"));
    assert_eq!(decoded[1].severity, None);
    assert_eq!(
        hover(&json!({"contents": {"kind": "markdown", "value": "**x**"}})).as_deref(),
        Some("**x**")
    );
    assert_eq!(
        hover(&json!({"contents": [{"language": "rust", "value": "fn f()"}, "doc"]})).as_deref(),
        Some("```rust\nfn f()\n```\n\ndoc")
    );
    assert_eq!(hover(&json!({"contents": ""})), None);
    assert_eq!(hover(&Value::Null), None);
}
