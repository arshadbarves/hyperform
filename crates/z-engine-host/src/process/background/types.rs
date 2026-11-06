//! Values exchanged with background-shell callers.

use std::path::PathBuf;
use std::sync::Arc;

use z_engine_protocol::{JobId, JobStatus};

use crate::process::env::EnvPolicy;
use crate::process::shell::ShellSpec;

/// Receives job events as they happen; throttling is the caller's job.
pub type JobEventSink = Arc<dyn Fn(JobEvent) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobEvent {
    /// One output line (with its terminator) from stdout or stderr.
    Output { id: JobId, text: String },
    Exited {
        id: JobId,
        status: JobStatus,
        exit_code: Option<i32>,
    },
}

#[derive(Debug, Clone)]
pub struct BackgroundSpec {
    pub command: String,
    pub cwd: PathBuf,
    pub shell: ShellSpec,
    pub env: EnvPolicy,
    /// Shown in job lists (usually the command line or a description).
    pub label: String,
    /// The agent that started the job.
    pub owner: String,
}

/// Output the model has not seen yet, plus the job's current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRead {
    /// New output since the previous read (only matching lines when
    /// filtered).
    pub output: String,
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    /// Unread bytes lost because the 1 MiB history overflowed.
    pub dropped_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSnapshot {
    pub id: JobId,
    pub label: String,
    pub owner: String,
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    /// Epoch milliseconds.
    pub started_at: u64,
    pub finished_at: Option<u64>,
    /// The last 4 KiB of output.
    pub tail: String,
}
