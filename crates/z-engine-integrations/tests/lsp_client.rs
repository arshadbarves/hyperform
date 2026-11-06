//! One language server client against the fake server: handshake, document
//! synchronization, raw requests in UTF-16, diagnostics freshness, failed
//! starts and shutdown.

mod support;

use std::path::Path;
use std::time::Duration;

use serde_json::json;
use support::{FAKE_LSP, at, fake_lsp, line_of, rust_project};
use z_engine_integrations::lsp::{SyncOutcome, path_to_uri};
use z_engine_integrations::{IntegrationError, LspClient, Severity};

#[tokio::test]
async fn syncs_documents_by_content_and_answers_in_utf16() {
    let (_dir, root) = rust_project();
    let client = LspClient::start(&fake_lsp(&[]), &root, Path::new(FAKE_LSP))
        .await
        .unwrap();
    assert!(client.supports("definitionProvider") && client.supports("callHierarchyProvider"));
    assert!(!client.supports("typeDefinitionProvider"));
    let lib = root.join("src/lib.rs");
    assert_eq!(client.sync(&lib).await.unwrap(), SyncOutcome::Opened);
    assert_eq!(client.sync(&lib).await.unwrap(), SyncOutcome::Unchanged);
    assert_eq!(client.open_documents(), 1);

    // The call to `helper` sits after a crab: UTF-16 offset = chars + 1.
    let (line, column) = at("let crab", "helper");
    let params = json!({
        "textDocument": {"uri": path_to_uri(&lib)},
        "position": {"line": line - 1, "character": column},
    });
    let raw = client
        .request("textDocument/definition", params)
        .await
        .unwrap();
    let (decl_line, _) = line_of(support::LIB_RS, "fn helper");
    assert_eq!(
        raw["range"]["start"],
        json!({"line": decl_line - 1, "character": 3})
    );

    std::fs::write(&lib, support::LIB_RS.replace("helper", "assist")).unwrap();
    assert_eq!(client.sync(&lib).await.unwrap(), SyncOutcome::Changed);
    assert!(client.document_text(&lib).unwrap().contains("fn assist"));
    client.shutdown().await;
    assert!(client.is_closed());
}

#[tokio::test]
async fn diagnostics_convert_utf16_columns_and_report_freshness() {
    let (_dir, root) = rust_project();
    let client = LspClient::start(&fake_lsp(&[]), &root, Path::new(FAKE_LSP))
        .await
        .unwrap();
    let lib = root.join("src/lib.rs");
    let report = client
        .diagnostics(&lib, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(report.fresh);
    assert_eq!(report.diagnostics.len(), 1);
    let diagnostic = &report.diagnostics[0];
    let (line, column) = at("ERROR marker", "ERROR");
    assert_eq!((diagnostic.line, diagnostic.column), (line, column));
    assert_eq!(diagnostic.end_column, column + 5);
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.code.as_deref(), Some("E0001"));
    assert_eq!(diagnostic.source.as_deref(), Some("fake"));
    let unchanged = client
        .diagnostics(&lib, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(unchanged.fresh, "already published for this content");

    let extra = root.join("src/extra.rs");
    std::fs::write(&extra, "fn later() {}\n// WARN: numeric code\n").unwrap();
    let warned = client
        .diagnostics(&extra, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(warned.diagnostics[0].severity, Severity::Warning);
    assert_eq!(warned.diagnostics[0].code.as_deref(), Some("7"));
    assert_eq!(
        (warned.diagnostics[0].line, warned.diagnostics[0].column),
        (2, 4)
    );
}

#[tokio::test]
async fn silence_is_reported_as_stale_not_clean() {
    let (_dir, root) = rust_project();
    let client = LspClient::start(&fake_lsp(&["--quiet"]), &root, Path::new(FAKE_LSP))
        .await
        .unwrap();
    let report = client
        .diagnostics(&root.join("src/lib.rs"), Duration::from_millis(200))
        .await
        .unwrap();
    assert!(!report.fresh);
    assert!(report.diagnostics.is_empty());
}

#[tokio::test]
async fn failed_starts_and_bad_files_are_typed_errors() {
    let (_dir, root) = rust_project();
    let crashed =
        LspClient::start(&fake_lsp(&["--crash-on-init"]), &root, Path::new(FAKE_LSP)).await;
    assert!(
        matches!(crashed, Err(IntegrationError::Disconnected(_))),
        "{crashed:?}"
    );
    let missing =
        LspClient::start(&fake_lsp(&[]), &root, Path::new("/nonexistent/zengine-lsp")).await;
    assert!(
        matches!(missing, Err(IntegrationError::Spawn { .. })),
        "{missing:?}"
    );

    let client = LspClient::start(&fake_lsp(&[]), &root, Path::new(FAKE_LSP))
        .await
        .unwrap();
    let gone = client.sync(&root.join("src/missing.rs")).await;
    assert!(
        matches!(gone, Err(IntegrationError::NotFound(_))),
        "{gone:?}"
    );
    let binary = root.join("src/blob.rs");
    std::fs::write(&binary, [0xff, 0xfe, 0x00, 0x80]).unwrap();
    let not_text = client.sync(&binary).await;
    assert!(
        matches!(not_text, Err(IntegrationError::Unsupported(_))),
        "{not_text:?}"
    );
}
