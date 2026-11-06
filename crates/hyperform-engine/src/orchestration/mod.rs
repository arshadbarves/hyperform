//! Multi-agent orchestration: the agent registry, the mapping from a
//! definition to a child run, foreground and background subagents,
//! worktree isolation with apply/discard, work-plan task helpers, and live
//! `AgentInfo` tracking.

mod blueprint;
mod child;
mod decide;
mod launch;
mod orchestra;
mod recover;
mod registry;
mod report;
mod sink;
mod spawn;
mod task_run;
mod tracker;
mod worktree;

pub(crate) use blueprint::model_for;
pub(crate) use decide::{apply_command, discard_command};
pub(crate) use orchestra::Orchestra;
pub(crate) use recover::prune_stale_worktrees;
pub(crate) use registry::{AgentRegistry, offered};
pub(crate) use spawn::spawn;
pub(crate) use task_run::{apply, drop_kept};
pub(crate) use tracker::AgentTracker;
