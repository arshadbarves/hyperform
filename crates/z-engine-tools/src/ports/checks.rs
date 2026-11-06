//! Project checks run as verification evidence.

use async_trait::async_trait;
use z_engine_protocol::{CheckKind, CheckRecord};

use crate::context::ToolCtx;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckSummary {
    pub id: String,
    pub label: String,
    pub kind: CheckKind,
    pub command: String,
}

#[async_trait]
pub trait CheckPort: Send + Sync {
    fn list(&self, ctx: &ToolCtx) -> Vec<CheckSummary>;

    async fn run(&self, ctx: &ToolCtx, check_id: &str) -> Result<CheckRecord, String>;
}
