//! The LSP manager's lifecycle against the fake server: lazy start,
//! routing failures (unknown extension, missing binary, missing root
//! markers), fallbacks, cached start failures, capability gaps, and
//! shutdown.

mod support;

use std::path::Path;

use support::{FAKE_LSP, fake_lsp, rust_project};
use z_engine_integrations::{IntegrationError, LspManager, LspServerSpec, LspServerState};

fn unsupported(result: Result<impl std::fmt::Debug, IntegrationError>) -> String {
    match result {
        Err(IntegrationError::Unsupported(reason)) => reason,
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[tokio::test]
async fn servers_start_lazily_and_report_status() {
    let (_dir, root) = rust_project();
    let manager = LspManager::new(&root, vec![fake_lsp(&[])]);
    assert!(
        manager.status().is_empty(),
        "nothing starts before a file is asked about"
    );
    let symbols = manager
        .document_symbols(Path::new("src/lib.rs"))
        .await
        .unwrap();
    assert!(!symbols.is_empty());
    let status = manager.status();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].name, "fake-lsp");
    assert_eq!(status[0].root, root);
    assert_eq!(status[0].state, LspServerState::Running);
    assert_eq!(status[0].open_documents, 1);
    assert!(
        status[0]
            .stderr_tail
            .iter()
            .any(|line| line.contains("fake-lsp: started"))
    );
    manager.shutdown_all().await;
    assert!(manager.status().is_empty());
}

#[tokio::test]
async fn routing_failures_explain_themselves() {
    let (_dir, root) = rust_project();
    std::fs::write(root.join("notes.xyz"), "x").unwrap();
    let manager = LspManager::new(&root, vec![fake_lsp(&[])]);
    let reason = unsupported(manager.definition(Path::new("notes.xyz"), 1, 1).await);
    assert!(
        reason.contains("no language server is configured for `.xyz`"),
        "{reason}"
    );

    let missing = LspServerSpec::new("ghost", "zengine-no-such-lsp", &[], &["rs"], &[]);
    let manager = LspManager::new(&root, vec![missing]);
    let reason = unsupported(manager.hover(Path::new("src/lib.rs"), 1, 1).await);
    assert!(reason.contains("not on PATH"), "{reason}");

    let unmarked = tempfile::tempdir().unwrap();
    std::fs::write(unmarked.path().join("main.rs"), "fn main() {}\n").unwrap();
    let manager = LspManager::new(unmarked.path(), vec![fake_lsp(&[])]);
    let reason = unsupported(manager.document_symbols(Path::new("main.rs")).await);
    assert!(reason.contains("needs Cargo.toml"), "{reason}");
    assert!(manager.status().is_empty());
}

#[tokio::test]
async fn the_first_installed_server_for_an_extension_wins() {
    let (_dir, root) = rust_project();
    let specs = vec![
        LspServerSpec::new("preferred", "zengine-no-such-lsp", &[], &["rs"], &[]),
        LspServerSpec::new("fallback", FAKE_LSP, &[], &["rs"], &[]),
    ];
    let manager = LspManager::new(&root, specs);
    manager
        .document_symbols(Path::new("src/lib.rs"))
        .await
        .unwrap();
    assert_eq!(manager.status()[0].name, "fallback");
}

#[tokio::test]
async fn start_failures_are_cached() {
    let (_dir, root) = rust_project();
    let manager = LspManager::new(&root, vec![fake_lsp(&["--crash-on-init"])]);
    let first = manager.document_symbols(Path::new("src/lib.rs")).await;
    assert!(
        matches!(first, Err(IntegrationError::Disconnected(_))),
        "{first:?}"
    );
    let reason = unsupported(manager.definition(Path::new("src/lib.rs"), 1, 1).await);
    assert!(reason.contains("unavailable"), "{reason}");
    let status = manager.status();
    assert!(
        matches!(&status[0].state, LspServerState::Failed(_)),
        "{status:?}"
    );
    assert!(
        status[0]
            .stderr_tail
            .iter()
            .any(|line| line.contains("crashing during initialize"))
    );
}

#[tokio::test]
async fn files_must_exist_inside_the_project_and_capabilities_are_checked() {
    let (_dir, root) = rust_project();
    let outside = tempfile::tempdir().unwrap();
    let stray = outside.path().join("stray.rs");
    std::fs::write(&stray, "fn stray() {}\n").unwrap();
    let manager = LspManager::new(&root, vec![fake_lsp(&["--minimal"])]);
    let reason = unsupported(manager.definition(&stray, 1, 1).await);
    assert!(reason.contains("outside the project"), "{reason}");
    let missing = manager.definition(Path::new("src/missing.rs"), 1, 1).await;
    assert!(
        matches!(missing, Err(IntegrationError::NotFound(_))),
        "{missing:?}"
    );
    let past_end = manager.definition(Path::new("src/lib.rs"), 500, 1).await;
    assert!(
        matches!(past_end, Err(IntegrationError::NotFound(_))),
        "{past_end:?}"
    );
    let reason = unsupported(manager.implementations(Path::new("src/lib.rs"), 1, 8).await);
    assert!(
        reason.contains("does not support go to implementation"),
        "{reason}"
    );
    let reason = unsupported(manager.incoming_calls(Path::new("src/lib.rs"), 11, 4).await);
    assert!(reason.contains("call hierarchy"), "{reason}");
}

#[tokio::test]
async fn workspace_symbols_start_servers_marked_in_the_project_root() {
    let (_dir, root) = rust_project();
    let manager = LspManager::new(&root, vec![fake_lsp(&[])]);
    let symbols = manager.workspace_symbols("gree").await.unwrap();
    assert_eq!(
        manager.status().len(),
        1,
        "started from the Cargo.toml marker"
    );
    // Nothing is open yet, so the fake knows no symbols; opening a file fills it.
    assert!(symbols.is_empty());
    manager
        .document_symbols(Path::new("src/lib.rs"))
        .await
        .unwrap();
    let names: Vec<String> = manager
        .workspace_symbols("gree")
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, ["Greeter", "greet"]);

    let empty = tempfile::tempdir().unwrap();
    let idle = LspManager::new(empty.path(), vec![fake_lsp(&[])]);
    let reason = unsupported(idle.workspace_symbols("x").await);
    assert!(reason.contains("no running language server"), "{reason}");
}
