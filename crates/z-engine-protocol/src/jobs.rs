//! Background jobs: shells started with `run_in_background` and background
//! agents. Both are polled with `JobOutput` and stopped with `JobKill`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AgentId, JobId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum JobKind {
    Shell,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum JobStatus {
    Running,
    Completed,
    Failed,
    Killed,
}

impl JobStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobInfo {
    pub job_id: JobId,
    pub kind: JobKind,
    /// Command line or agent description.
    pub label: String,
    /// Agent that started the job.
    pub owner: AgentId,
    /// For agent jobs, the background agent's id.
    pub agent_id: Option<AgentId>,
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    #[ts(type = "number")]
    pub started_at: u64,
    #[ts(type = "number | null")]
    pub finished_at: Option<u64>,
    /// Last few KiB of output for the jobs panel.
    pub output_tail: String,
}
