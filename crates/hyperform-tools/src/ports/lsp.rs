//! Language-server queries behind the `LSP` tool.

use async_trait::async_trait;

use crate::context::ToolCtx;

/// One validated query. `file_path` is absolute; `line` and `character`
/// are 1-based, as shown by `Read` and editors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRequest {
    /// `definition`, `references`, `hover`, `documentSymbols`,
    /// `workspaceSymbols`, `implementations`, `incomingCalls`,
    /// `outgoingCalls`, `diagnostics`, or `renamePreview`.
    pub operation: String,
    pub file_path: Option<String>,
    pub line: Option<u32>,
    pub character: Option<u32>,
    pub query: Option<String>,
    pub new_name: Option<String>,
}

#[async_trait]
pub trait LspPort: Send + Sync {
    /// The formatted answer for the model.
    async fn query(&self, ctx: &ToolCtx, req: LspRequest) -> Result<String, String>;
}
