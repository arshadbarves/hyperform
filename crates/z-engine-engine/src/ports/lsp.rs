//! `LspPort`: `LSP` tool queries answered by the session's language
//! servers, with paths shown relative to the agent's root.

use std::sync::Arc;

use async_trait::async_trait;
use z_engine_tools::{LspPort, LspRequest, ToolCtx};

use crate::lsp::{LspWorker, answer};

#[derive(Debug)]
pub(crate) struct Lsp {
    worker: Arc<LspWorker>,
}

impl Lsp {
    pub(crate) fn new(worker: Arc<LspWorker>) -> Self {
        Self { worker }
    }
}

#[async_trait]
impl LspPort for Lsp {
    async fn query(&self, ctx: &ToolCtx, req: LspRequest) -> Result<String, String> {
        let root = ctx.root.clone();
        self.worker
            .run(move |manager| answer(manager, root, req))
            .await
            .unwrap_or_else(|| Err("the language servers of this session have stopped".into()))
    }
}
