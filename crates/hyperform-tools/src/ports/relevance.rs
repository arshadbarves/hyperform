//! Relevance of parts of a tool result to the agent's current task,
//! answered by the session's decision model. The engine provides the port
//! only while `decisions_output_trim` or `decisions_search_rank` runs, so
//! with both off tools behave exactly as before.

use async_trait::async_trait;

use crate::context::ToolCtx;

/// What is ranked; each target is its own experimental feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankTarget {
    /// 40-line windows from the middle of long command output.
    OutputWindows,
    /// Search results past the result cap.
    SearchHits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankRequest {
    pub target: RankTarget,
    /// The tool asking (`Bash`, `Grep`, `Glob`).
    pub tool: String,
    /// What ran: the command, or the search pattern.
    pub subject: String,
    /// One short excerpt per item, in result order.
    pub items: Vec<String>,
    /// Characters of the result the ranking could leave out, for traces.
    pub chars: usize,
}

#[async_trait]
pub trait RelevancePort: Send + Sync {
    /// Whether `target` runs in this session; tools skip building items
    /// when it does not.
    fn ranks(&self, target: RankTarget) -> bool;

    /// Per item, `Some(true)` relevant, `Some(false)` not, `None` unknown.
    /// `None` overall (shadow mode, no answer in time, every answer
    /// unsure) means keep today's behavior.
    async fn rank(&self, ctx: &ToolCtx, request: RankRequest) -> Option<Vec<Option<bool>>>;
}
