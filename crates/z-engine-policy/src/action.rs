//! What a tool call wants to do. Tools derive an [`Action`] from their input
//! and the policy decides on the action, never on raw tool input.

use std::path::PathBuf;

/// The effect of one tool call, in the terms permission rules use.
///
/// Paths should be absolute and already resolved by the caller: the policy
/// compares them lexically and never touches the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Reads files or directories (`Read`, `Glob`, `Grep`, `LSP` queries).
    /// An empty list stands for the project root.
    Read { paths: Vec<PathBuf> },
    /// Creates or changes files (`Write`, `Edit`, `MultiEdit`,
    /// `NotebookEdit`, `ApplyAgentChanges`).
    Write { paths: Vec<PathBuf> },
    /// Runs a shell command line (`Bash`, verification runs, `!cmd`).
    Execute { command: String },
    /// Fetches a URL (`WebFetch`).
    Fetch { url: String },
    /// Searches the web (`WebSearch`).
    Search,
    /// Starts a subagent (`Agent`); its own tool calls are decided separately.
    Agent { agent_type: String },
    /// Calls a tool on an MCP server.
    Mcp {
        server: String,
        tool: String,
        read_only: bool,
    },
    /// Loads a skill's instructions (`Skill`), so `Skill(name)` rules can match.
    Skill { name: String },
    /// Any other tool: `TodoWrite`, `AskUserQuestion`, `ExitPlanMode` and
    /// `JobOutput` are read-only, `JobKill` is not.
    Other { read_only: bool },
}

impl Action {
    /// Changes state, so plan mode refuses it.
    pub(crate) fn mutates(&self) -> bool {
        match self {
            Self::Write { .. } => true,
            Self::Execute { command } => !crate::shell::is_read_only(command),
            Self::Mcp { read_only, .. } | Self::Other { read_only } => !read_only,
            Self::Read { .. }
            | Self::Fetch { .. }
            | Self::Search
            | Self::Agent { .. }
            | Self::Skill { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_state_changes_mutate() {
        let mutating = [
            Action::Write { paths: vec![] },
            Action::Execute {
                command: "cargo test".into(),
            },
            Action::Mcp {
                server: "github".into(),
                tool: "create_issue".into(),
                read_only: false,
            },
            Action::Other { read_only: false },
        ];
        for action in mutating {
            assert!(action.mutates(), "{action:?}");
        }
        let inert = [
            Action::Read { paths: vec![] },
            Action::Execute {
                command: "git status".into(),
            },
            Action::Fetch {
                url: "https://example.com".into(),
            },
            Action::Search,
            Action::Agent {
                agent_type: "explore".into(),
            },
            Action::Mcp {
                server: "github".into(),
                tool: "get_issue".into(),
                read_only: true,
            },
            Action::Skill {
                name: "review".into(),
            },
            Action::Other { read_only: true },
        ];
        for action in inert {
            assert!(!action.mutates(), "{action:?}");
        }
    }
}
