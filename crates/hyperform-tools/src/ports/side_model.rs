//! The fast side model that answers `WebFetch` prompts from page content.

use async_trait::async_trait;

use crate::context::ToolCtx;

#[async_trait]
pub trait SideModelPort: Send + Sync {
    /// Answers `prompt` from `content` (the page with its URL header).
    async fn extract(
        &self,
        ctx: &ToolCtx,
        content: String,
        prompt: String,
    ) -> Result<String, String>;
}
