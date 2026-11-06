//! LSP operations in the model's coordinates (1-based lines, character
//! columns): navigation, symbols, hover, call hierarchy, diagnostics, and
//! rename previews that are never applied.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::client::LspClient;
use super::convert;
use super::manager::LspManager;
use super::position::to_lsp;
use super::resolve::Texts;
use super::types::{
    CallDirection, CallEdge, FileDiagnostics, Hover, Location, SymbolNode, WorkspaceEditPlan,
    WorkspaceSymbol,
};
use super::uri::path_to_uri;
use crate::error::IntegrationError;

/// How long [`LspManager::diagnostics`] waits for a fresh publish.
pub const DIAGNOSTICS_WAIT: Duration = Duration::from_secs(3);

/// A synced file on its server.
struct Target {
    client: Arc<LspClient>,
    path: PathBuf,
    text: Arc<str>,
}

impl Target {
    fn document(&self) -> Value {
        json!({"uri": path_to_uri(&self.path)})
    }

    fn position(&self, line: u32, column: u32) -> Result<Value, IntegrationError> {
        let position = to_lsp(&self.text, line, column).ok_or_else(|| {
            IntegrationError::NotFound(format!(
                "line {line} of {} (the file has {} lines)",
                self.path.display(),
                self.text.split('\n').count()
            ))
        })?;
        Ok(json!({"textDocument": self.document(), "position": position}))
    }

    fn require(&self, provider: &str, what: &str) -> Result<(), IntegrationError> {
        if self.client.supports(provider) {
            return Ok(());
        }
        Err(IntegrationError::Unsupported(format!(
            "{} does not support {what}",
            self.client.name()
        )))
    }

    fn texts(&self) -> Texts<'_> {
        Texts::new(self.client.documents())
    }
}

impl LspManager {
    async fn target(&self, path: &Path) -> Result<Target, IntegrationError> {
        let path = self.resolve_file(path).await?;
        let client = self.client_for(&path).await?;
        client.sync(&path).await?;
        let text = client
            .document_text(&path)
            .ok_or_else(|| IntegrationError::NotFound(path.display().to_string()))?;
        Ok(Target { client, path, text })
    }

    async fn locate(
        &self,
        path: &Path,
        (line, column): (u32, u32),
        method: &str,
        (provider, what): (&str, &str),
    ) -> Result<Vec<Location>, IntegrationError> {
        let target = self.target(path).await?;
        target.require(provider, what)?;
        let mut params = target.position(line, column)?;
        if method == "textDocument/references" {
            params["context"] = json!({"includeDeclaration": true});
        }
        let raw = convert::locations(&target.client.request(method, params).await?)?;
        let mut texts = target.texts();
        texts
            .load_all(raw.iter().map(|location| &location.path))
            .await;
        Ok(raw
            .iter()
            .map(|location| texts.location(location))
            .collect())
    }

    pub async fn definition(
        &self,
        path: &Path,
        line: u32,
        column: u32,
    ) -> Result<Vec<Location>, IntegrationError> {
        let capability = ("definitionProvider", "go to definition");
        self.locate(path, (line, column), "textDocument/definition", capability)
            .await
    }

    /// References including the declaration.
    pub async fn references(
        &self,
        path: &Path,
        line: u32,
        column: u32,
    ) -> Result<Vec<Location>, IntegrationError> {
        let capability = ("referencesProvider", "find references");
        self.locate(path, (line, column), "textDocument/references", capability)
            .await
    }

    pub async fn implementations(
        &self,
        path: &Path,
        line: u32,
        column: u32,
    ) -> Result<Vec<Location>, IntegrationError> {
        let capability = ("implementationProvider", "go to implementation");
        self.locate(
            path,
            (line, column),
            "textDocument/implementation",
            capability,
        )
        .await
    }

    pub async fn hover(
        &self,
        path: &Path,
        line: u32,
        column: u32,
    ) -> Result<Option<Hover>, IntegrationError> {
        let target = self.target(path).await?;
        target.require("hoverProvider", "hover")?;
        let result = target
            .client
            .request("textDocument/hover", target.position(line, column)?)
            .await?;
        Ok(convert::hover(&result).map(|contents| Hover { contents }))
    }

    pub async fn document_symbols(&self, path: &Path) -> Result<Vec<SymbolNode>, IntegrationError> {
        let target = self.target(path).await?;
        target.require("documentSymbolProvider", "document symbols")?;
        let params = json!({"textDocument": target.document()});
        let raw = convert::document_symbols(
            &target
                .client
                .request("textDocument/documentSymbol", params)
                .await?,
        )?;
        let mut texts = target.texts();
        texts.load(&target.path).await;
        Ok(texts.symbols(&target.path, &raw))
    }

    /// Symbols matching `query` from every running server; when none runs
    /// yet, servers marked in the project root are started first.
    pub async fn workspace_symbols(
        &self,
        query: &str,
    ) -> Result<Vec<WorkspaceSymbol>, IntegrationError> {
        let clients: Vec<_> = self
            .workspace_clients()
            .await
            .into_iter()
            .filter(|client| client.supports("workspaceSymbolProvider"))
            .collect();
        if clients.is_empty() {
            return Err(IntegrationError::Unsupported(
                "no running language server offers workspace symbols; ask about a source file first so its server starts".into(),
            ));
        }
        let mut symbols = Vec::new();
        let mut failure = None;
        let mut answered = false;
        for client in &clients {
            let result = client
                .request("workspace/symbol", json!({"query": query}))
                .await;
            match result.and_then(|result| convert::workspace_symbols(&result)) {
                Ok(raw) => {
                    answered = true;
                    let mut texts = Texts::new(client.documents());
                    texts
                        .load_all(raw.iter().map(|symbol| &symbol.location.path))
                        .await;
                    symbols.extend(raw.iter().map(|symbol| texts.workspace_symbol(symbol)));
                }
                Err(error) => {
                    tracing::warn!(server = %client.name(), %error, "workspace/symbol failed");
                    failure = Some(error);
                }
            }
        }
        match (answered, failure) {
            (false, Some(error)) => Err(error),
            _ => Ok(symbols),
        }
    }

    pub async fn incoming_calls(
        &self,
        path: &Path,
        line: u32,
        column: u32,
    ) -> Result<Vec<CallEdge>, IntegrationError> {
        self.calls(path, line, column, CallDirection::Incoming)
            .await
    }

    pub async fn outgoing_calls(
        &self,
        path: &Path,
        line: u32,
        column: u32,
    ) -> Result<Vec<CallEdge>, IntegrationError> {
        self.calls(path, line, column, CallDirection::Outgoing)
            .await
    }

    async fn calls(
        &self,
        path: &Path,
        line: u32,
        column: u32,
        direction: CallDirection,
    ) -> Result<Vec<CallEdge>, IntegrationError> {
        let target = self.target(path).await?;
        target.require("callHierarchyProvider", "call hierarchy")?;
        let prepare = target.position(line, column)?;
        let items = convert::call_items(
            &target
                .client
                .request("textDocument/prepareCallHierarchy", prepare)
                .await?,
        )?;
        if items.is_empty() {
            return Err(IntegrationError::NotFound(format!(
                "no function at {}:{line}:{column}",
                target.path.display()
            )));
        }
        let (method, key) = match direction {
            CallDirection::Incoming => ("callHierarchy/incomingCalls", "from"),
            CallDirection::Outgoing => ("callHierarchy/outgoingCalls", "to"),
        };
        let mut raw = Vec::new();
        for item in &items {
            let result = target
                .client
                .request(method, json!({"item": item.wire}))
                .await?;
            for call in convert::calls(&result, key)? {
                let sites_in = match direction {
                    CallDirection::Incoming => call.item.location.path.clone(),
                    CallDirection::Outgoing => item.location.path.clone(),
                };
                raw.push((call, sites_in));
            }
        }
        let mut texts = target.texts();
        for (call, sites_in) in &raw {
            texts.load(&call.item.location.path).await;
            texts.load(sites_in).await;
        }
        Ok(raw
            .iter()
            .map(|(call, sites_in)| texts.call_edge(call, sites_in))
            .collect())
    }

    /// Syncs the file, then waits up to [`DIAGNOSTICS_WAIT`] for diagnostics
    /// of its current content.
    pub async fn diagnostics(&self, path: &Path) -> Result<FileDiagnostics, IntegrationError> {
        let path = self.resolve_file(path).await?;
        let client = self.client_for(&path).await?;
        client.diagnostics(&path, DIAGNOSTICS_WAIT).await
    }

    /// The edits renaming the symbol at the position would make. Nothing is
    /// written; the caller previews, approves and applies.
    pub async fn rename_preview(
        &self,
        path: &Path,
        line: u32,
        column: u32,
        new_name: &str,
    ) -> Result<WorkspaceEditPlan, IntegrationError> {
        let target = self.target(path).await?;
        target.require("renameProvider", "rename")?;
        let mut params = target.position(line, column)?;
        params["newName"] = Value::String(new_name.to_string());
        let result = target.client.request("textDocument/rename", params).await?;
        if result.is_null() {
            return Err(IntegrationError::NotFound(format!(
                "no renameable symbol at {}:{line}:{column}",
                target.path.display()
            )));
        }
        let raw = convert::workspace_edit(&result)?;
        let mut texts = target.texts();
        texts.load_all(raw.keys()).await;
        Ok(texts.edit_plan(&raw))
    }
}
