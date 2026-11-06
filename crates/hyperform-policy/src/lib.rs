//! Pure permission engine for tool calls: allow/ask/deny rules, permission
//! modes, and static shell-command analysis. No filesystem, process, or
//! network access; paths are compared lexically.

mod action;
mod decide;
mod engine;
mod error;
mod paths;
mod rules;
pub mod shell;

pub use action::Action;
pub use engine::{Decision, Policy, PolicyConfig, PolicyContext};
pub use error::PolicyError;
pub use rules::{Rule, RuleKind};
pub use shell::{ShellAnalysis, analyze, is_common_fs_command, is_read_only, suggest_prefix};
