//! One open session: shared core, event path, log, status, state, jobs
//! (shells and background agents), the actor that serializes GUI
//! commands, and everything it runs.

mod actor;
mod agent_jobs;
mod checkpoint;
mod command_turn;
mod compaction;
mod control;
mod core;
mod emitter;
mod grants;
mod handle;
mod jobs;
mod journal;
mod meta;
mod open;
mod prompt;
mod reload;
mod remember;
mod reminders;
mod reports;
mod resume;
mod rewind;
mod shell;
mod slash;
mod snapshot;
mod state;
mod status;
mod title;
mod tools;
mod trust;
mod turn;

pub(crate) use core::{AgentResources, SessionCore, Shared};
pub(crate) use emitter::Emitter;
pub(crate) use handle::SessionHandle;
pub(crate) use jobs::JobHub;
pub(crate) use journal::Journal;
pub(crate) use open::open_session;
pub(crate) use reload::git_info;
pub(crate) use reminders::ReminderBox;
pub(crate) use reports::context_report;
pub(crate) use snapshot::emit_snapshot;
pub(crate) use state::SessionState;
pub(crate) use status::StatusTracker;
pub(crate) use tools::rebuild_tools;
