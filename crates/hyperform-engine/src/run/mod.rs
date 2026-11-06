//! Agent runs: the round loop shared by the main agent and subagents,
//! with request assembly, streaming, context pressure, compaction, the
//! stop boundary, reminders and usage accounting.

mod agent;
mod budget;
mod compact;
mod meter;
mod mutation;
mod pressure;
mod reminders;
mod repo_map;
mod request;
mod side;
mod sink;
mod spec;
mod stop;
mod stream;
mod tally;
mod usage;
mod web;

pub(crate) use agent::AgentRun;
pub(crate) use compact::{CompactJob, summarize};
pub(crate) use meter::ContextMeter;
pub(crate) use pressure::MIN_CLEAR_CHARS;
pub(crate) use reminders::collect as collect_reminders;
pub(crate) use repo_map::{RepoMapCache, prebuild_repo_map, repo_map_files};
pub(crate) use request::{inspect as inspect_request, prepare as prepare_request};
pub(crate) use side::side_request;
pub(crate) use sink::{MainSink, TranscriptSink};
pub(crate) use spec::{AgentSpec, ModelChoice, RunContext, RunOutcome, ToolFilter, WorktreeScope};
pub(crate) use tally::ChildTally;
pub(crate) use web::{WebTaint, reads_web};
