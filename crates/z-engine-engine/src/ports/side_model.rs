//! `SideModelPort`: `WebFetch` prompts answered by the fast model from the
//! fetched page; usage is recorded as a side request.

use std::sync::Arc;

use async_trait::async_trait;
use z_engine_llm::{ModelRequest, SystemBlock};
use z_engine_prompts::auxiliary::WEB_EXTRACT;
use z_engine_protocol::Message;
use z_engine_tools::{SideModelPort, ToolCtx};

use crate::run::side_request;
use crate::session::SessionCore;
use crate::settings::models::fast_model;

const EXTRACT_MAX_TOKENS: u32 = 4_096;

#[derive(Debug)]
pub(crate) struct SideModel {
    core: Arc<SessionCore>,
}

impl SideModel {
    pub(crate) fn new(core: Arc<SessionCore>) -> Self {
        Self { core }
    }
}

#[async_trait]
impl SideModelPort for SideModel {
    async fn extract(
        &self,
        ctx: &ToolCtx,
        content: String,
        prompt: String,
    ) -> Result<String, String> {
        let model = fast_model(&self.core.settings().settings, &self.core.main_model());
        let question = format!("{content}\n\nQuestion: {}", prompt.trim());
        let request = ModelRequest::new(model, vec![Message::user_text(question)])
            .with_system(vec![SystemBlock::new(WEB_EXTRACT)])
            .with_max_tokens(EXTRACT_MAX_TOKENS);
        let turn = side_request(&self.core, &ctx.agent_id, request, &ctx.cancel)
            .await
            .map_err(|error| error.to_string())?;
        let answer = turn.text();
        if answer.trim().is_empty() {
            return Err("the extraction model returned no text".to_string());
        }
        Ok(answer)
    }
}
