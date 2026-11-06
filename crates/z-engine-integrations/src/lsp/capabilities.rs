//! What this client tells a language server in `initialize`, and how it
//! reads the server's answer. Only UTF-16 positions are offered, full-text
//! synchronization is used, and workspace edits must be text-only.

use std::path::Path;

use serde_json::{Value, json};

use super::uri::path_to_uri;
use crate::error::IntegrationError;

pub(crate) fn initialize_params(root: &Path) -> Value {
    let uri = path_to_uri(root);
    let name = root.file_name().map_or_else(
        || "workspace".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    json!({
        "processId": std::process::id(),
        "clientInfo": {"name": "z-engine", "version": env!("CARGO_PKG_VERSION")},
        "rootUri": uri,
        "rootPath": root.display().to_string(),
        "workspaceFolders": [{"uri": uri, "name": name}],
        "capabilities": client_capabilities(),
    })
}

fn client_capabilities() -> Value {
    let symbol_kinds: Vec<u32> = (1..=26).collect();
    json!({
        "general": {"positionEncodings": ["utf-16"]},
        "workspace": {
            "configuration": true,
            "workspaceFolders": false,
            "symbol": {"dynamicRegistration": false, "symbolKind": {"valueSet": symbol_kinds}},
            "workspaceEdit": {"documentChanges": true},
        },
        "textDocument": {
            "synchronization": {"dynamicRegistration": false, "didSave": true, "willSave": false},
            "hover": {"dynamicRegistration": false, "contentFormat": ["markdown", "plaintext"]},
            "definition": {"dynamicRegistration": false, "linkSupport": true},
            "references": {"dynamicRegistration": false},
            "implementation": {"dynamicRegistration": false, "linkSupport": true},
            "documentSymbol": {
                "dynamicRegistration": false,
                "hierarchicalDocumentSymbolSupport": true,
                "symbolKind": {"valueSet": symbol_kinds},
            },
            "callHierarchy": {"dynamicRegistration": false},
            "rename": {"dynamicRegistration": false, "prepareSupport": false},
            "publishDiagnostics": {"relatedInformation": false, "versionSupport": false},
        },
        "window": {"workDoneProgress": true},
    })
}

/// The server capabilities of an `initialize` result, after checking that
/// the server kept to UTF-16 positions.
pub(crate) fn server_capabilities(result: &Value, server: &str) -> Result<Value, IntegrationError> {
    let capabilities = result
        .get("capabilities")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| {
            IntegrationError::Protocol(format!("`{server}` returned no capabilities"))
        })?;
    match capabilities.get("positionEncoding").and_then(Value::as_str) {
        None | Some("utf-16") => Ok(capabilities),
        Some(other) => Err(IntegrationError::Protocol(format!(
            "`{server}` chose position encoding `{other}`, but only utf-16 was offered"
        ))),
    }
}

/// Whether a `...Provider` capability is present and not `false`.
pub(crate) fn provides(capabilities: &Value, provider: &str) -> bool {
    match capabilities.get(provider) {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(_) => true,
    }
}

/// Whether the server wants `didSave` notifications.
pub(crate) fn wants_save(capabilities: &Value) -> bool {
    match capabilities.get("textDocumentSync") {
        Some(Value::Object(sync)) => sync
            .get("save")
            .is_some_and(|save| save != &Value::Bool(false)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_offers_utf16_and_hierarchical_symbols() {
        let params = initialize_params(Path::new("/work/demo"));
        assert_eq!(params["rootUri"], "file:///work/demo");
        assert_eq!(params["workspaceFolders"][0]["name"], "demo");
        let caps = &params["capabilities"];
        assert_eq!(caps["general"]["positionEncodings"], json!(["utf-16"]));
        assert_eq!(
            caps["textDocument"]["documentSymbol"]["hierarchicalDocumentSymbolSupport"],
            true
        );
        assert!(caps["workspace"]["symbol"].is_object());
    }

    #[test]
    fn reads_server_capabilities() {
        let result = json!({"capabilities": {
            "hoverProvider": true, "renameProvider": {"prepareProvider": true},
            "implementationProvider": false, "textDocumentSync": {"openClose": true, "change": 1, "save": {}}
        }});
        let caps = server_capabilities(&result, "fake").unwrap();
        assert!(provides(&caps, "hoverProvider") && provides(&caps, "renameProvider"));
        assert!(
            !provides(&caps, "implementationProvider") && !provides(&caps, "callHierarchyProvider")
        );
        assert!(wants_save(&caps));
        assert!(!wants_save(&json!({"textDocumentSync": 1})));
        let utf8 = json!({"capabilities": {"positionEncoding": "utf-8"}});
        assert!(matches!(
            server_capabilities(&utf8, "x"),
            Err(IntegrationError::Protocol(_))
        ));
        assert!(server_capabilities(&json!({}), "x").is_err());
    }
}
