//! LSP navigation through the manager with the model's 1-based character
//! positions, including multibyte text, and the text rendered for the model.

mod support;

use std::path::Path;

use support::{at, fake_lsp, line_of, rust_project};
use z_engine_integrations::lsp::format::{
    format_calls, format_diagnostics, format_hover, format_locations, format_rename_plan,
    format_symbols, format_workspace_symbols,
};
use z_engine_integrations::{CallDirection, LspManager};

const LIB: &str = "src/lib.rs";

fn manager(root: &Path) -> LspManager {
    LspManager::new(root, vec![fake_lsp(&[])])
}

#[tokio::test]
async fn definition_references_and_hover_across_a_crab() {
    let (_dir, root) = rust_project();
    let manager = manager(&root);
    let (line, column) = at("let crab", "helper");
    let (decl, decl_text) = line_of(support::LIB_RS, "fn helper");

    let definition = manager
        .definition(Path::new(LIB), line, column)
        .await
        .unwrap();
    assert_eq!(definition.len(), 1);
    assert_eq!((definition[0].line, definition[0].column), (decl, 4));
    assert_eq!(definition[0].path, root.join(LIB));
    assert_eq!(
        format_locations(&root, &definition),
        format!("src/lib.rs:{decl}:4: {}", decl_text.trim())
    );

    let references = manager.references(Path::new(LIB), decl, 4).await.unwrap();
    let spots: Vec<(u32, u32)> = references.iter().map(|l| (l.line, l.column)).collect();
    assert_eq!(spots, [(line, column), (decl, 4)]);
    assert_eq!(references[0].end_column, column + 6);

    let hover = manager.hover(Path::new(LIB), line, column).await.unwrap();
    assert_eq!(
        format_hover(hover.as_ref()),
        "```rust\nfn helper(g: &Greeter) -> String\n```"
    );
    let (blank, _) = line_of(support::LIB_RS, "fn main");
    let nothing = manager.hover(Path::new(LIB), blank - 1, 1).await.unwrap();
    assert_eq!(nothing, None);
}

#[tokio::test]
async fn implementations_and_symbols() {
    let (_dir, root) = rust_project();
    let manager = manager(&root);
    let implementations = manager.implementations(Path::new(LIB), 1, 8).await.unwrap();
    let (impl_line, _) = line_of(support::LIB_RS, "impl Greeter");
    assert_eq!(
        (implementations[0].line, implementations[0].column),
        (impl_line, 6)
    );

    let symbols = manager.document_symbols(Path::new(LIB)).await.unwrap();
    assert_eq!(
        format_symbols(&symbols),
        "struct Greeter (lines 1-3)\n  field name (line 2)\n\
         object impl Greeter (lines 5-9)\n  method new (lines 6-8)\n\
         function greet (lines 11-13)\nfunction helper (lines 15-17)\nfunction main (lines 19-22)"
    );
    assert_eq!((symbols[0].line, symbols[0].column), (1, 8));

    let found = manager.workspace_symbols("helper").await.unwrap();
    assert_eq!(
        format_workspace_symbols(&root, &found),
        "function helper in fake - src/lib.rs:15:4"
    );
}

#[tokio::test]
async fn call_hierarchy_in_both_directions() {
    let (_dir, root) = rust_project();
    let manager = manager(&root);
    let (call_line, call_column) = at("let crab", "helper");
    let (greet_line, _) = line_of(support::LIB_RS, "fn greet");

    let callers = manager.incoming_calls(Path::new(LIB), 15, 4).await.unwrap();
    assert_eq!(callers.len(), 1);
    assert_eq!(callers[0].item.name, "greet");
    assert_eq!(
        (
            callers[0].item.location.line,
            callers[0].item.location.column
        ),
        (greet_line, 4)
    );
    assert_eq!(
        (callers[0].sites[0].line, callers[0].sites[0].column),
        (call_line, call_column)
    );
    let rendered = format_calls(&root, &callers, CallDirection::Incoming);
    assert!(rendered.starts_with(&format!("function greet - src/lib.rs:{greet_line}:4\n  called at src/lib.rs:{call_line}:{call_column}: ")), "{rendered}");

    let callees = manager
        .outgoing_calls(Path::new(LIB), greet_line, 4)
        .await
        .unwrap();
    assert_eq!(callees.len(), 1);
    assert_eq!(callees[0].item.name, "helper");
    assert_eq!(
        (callees[0].sites[0].line, callees[0].sites[0].column),
        (call_line, call_column)
    );

    let (new_line, new_text) = line_of(support::LIB_RS, "fn new");
    let constructors = manager
        .incoming_calls(Path::new(LIB), new_line, support::col(&new_text, "new"))
        .await
        .unwrap();
    assert_eq!(constructors[0].item.name, "main");
    let not_a_function = manager.incoming_calls(Path::new(LIB), 2, 5).await;
    assert!(not_a_function.is_err());
}

#[tokio::test]
async fn diagnostics_and_rename_previews() {
    let (_dir, root) = rust_project();
    let manager = manager(&root);
    let report = manager.diagnostics(Path::new(LIB)).await.unwrap();
    let (line, column) = at("ERROR marker", "ERROR");
    assert_eq!(
        format_diagnostics(&root, &report),
        format!("src/lib.rs:{line}:{column}: error: found an ERROR marker [E0001]")
    );

    let (call_line, call_column) = at("let crab", "helper");
    let plan = manager
        .rename_preview(Path::new(LIB), call_line, call_column, "assist")
        .await
        .unwrap();
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.edit_count(), 2);
    let edits: Vec<(u32, u32, u32)> = plan.files[0]
        .edits
        .iter()
        .map(|e| (e.start_line, e.start_col, e.end_col))
        .collect();
    assert_eq!(
        edits,
        [(call_line, call_column, call_column + 6), (15, 4, 10)]
    );
    assert_eq!(
        format_rename_plan(&root, &plan),
        format!(
            "Rename preview (not applied): 2 edits in 1 file\nsrc/lib.rs (2 edits)\n  \
             {call_line}:{call_column}-{call_line}:{} -> \"assist\"\n  15:4-15:10 -> \"assist\"",
            call_column + 6
        )
    );
    let on_disk = std::fs::read_to_string(root.join(LIB)).unwrap();
    assert_eq!(on_disk, support::LIB_RS, "a preview never writes");
}
