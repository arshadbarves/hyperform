//! Background jobs: shells started with `run_in_background` and background
//! agents, read with `JobOutput` and stopped with `JobKill`.

use std::time::Duration;

use async_trait::async_trait;
use z_engine_protocol::{JobId, JobKind, JobStatus};

use crate::context::ToolCtx;

/// Output the model has not seen yet, with the job's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutput {
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    pub output: String,
    pub kind: JobKind,
}

#[async_trait]
pub trait JobPort: Send + Sync {
    async fn spawn_shell(
        &self,
        ctx: &ToolCtx,
        command: String,
        description: Option<String>,
    ) -> Result<JobId, String>;

    /// New output since the last read; `filter` is a validated regex and
    /// `wait` bounds how long to wait for the job to finish first.
    async fn output(
        &self,
        ctx: &ToolCtx,
        job: &JobId,
        filter: Option<String>,
        wait: Option<Duration>,
    ) -> Result<JobOutput, String>;

    async fn kill(&self, ctx: &ToolCtx, job: &JobId) -> Result<(), String>;
}
