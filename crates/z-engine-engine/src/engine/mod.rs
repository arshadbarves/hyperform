//! The public engine API plus the catalog and export it serves.

mod api;
mod catalog;
mod export;
mod queries;

pub use api::Engine;
pub use queries::{
    AgentCard, ChangedKind, ChangedPath, CopilotDeviceCode, CopilotLoginPoll, DecisionModelStatus,
    DecisionModelTest, GitChangedFile, McpTestReport, RepoSummary, RunningFeature,
    SessionDecisions, SlashCommandInfo, SlashKind, TrustReport,
};
