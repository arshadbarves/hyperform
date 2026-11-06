//! Read-mostly queries the desktop GUI needs beside sessions: review-panel
//! diffs (session and git scope), the sidebar's repository summary,
//! worktrees, the context card's token breakdown, agent and slash-command
//! catalogs, `@file` search, MCP server tests, workspace trust, and the
//! decision trace and decision-model test, the native decision model's
//! download, GitHub Copilot sign-in, and work plans (plans, task packets
//! and why a task changed). Results serialize camelCase for the GUI.

mod catalogs;
mod changes;
mod context;
mod copilot;
mod decision_model;
mod decisions;
mod files;
mod git;
mod mcp;
#[cfg(test)]
mod testing;
mod trust;
mod work;

pub use catalogs::{AgentCard, SlashCommandInfo, SlashKind};
pub use changes::{ChangedKind, ChangedPath};
pub use copilot::{CopilotDeviceCode, CopilotLoginPoll};
pub use decision_model::DecisionModelStatus;
pub use decisions::{DecisionModelTest, RunningFeature, SessionDecisions};
pub use git::{GitChangedFile, RepoSummary};
pub use mcp::McpTestReport;
pub use trust::TrustReport;
