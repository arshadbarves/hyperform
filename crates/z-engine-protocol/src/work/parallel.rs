//! Which ready tasks may run at the same time, and why the others may not.
//! The kernel computes it; the engine dispatches only eligible tasks.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::TaskId;
use crate::work::plan::ParallelPolicy;

/// Why a task cannot start in parallel now. `task_id` names the task that
/// causes it: the task itself, or one it would run beside.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ParallelBlock {
    /// The session is in plan mode.
    Planning,
    /// Reconciliation findings affect the task (or a task it depends on):
    /// it is held until the user accepts the plan change.
    Replanning,
    /// The plan's parallel policy is not `approved`.
    NotApproved { parallel: ParallelPolicy },
    /// The task, or one it would run beside, must run alone.
    RunAlone { task_id: TaskId },
    /// The task, or one it would run beside, declares no files.
    UnknownScope { task_id: TaskId },
    /// The task's scope names `[work] serial_paths`.
    SerialPaths {
        task_id: TaskId,
        patterns: Vec<String>,
    },
    /// A dependency path links it to `task_id`.
    Ordered { task_id: TaskId },
    /// Its scope overlaps `task_id`'s.
    Overlap {
        task_id: TaskId,
        #[serde(default)]
        files: Vec<String>,
        #[serde(default)]
        symbols: Vec<String>,
    },
    /// `max` helpers already run or were chosen.
    NoSlot { max: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParallelExcluded {
    pub task_id: TaskId,
    pub block: ParallelBlock,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParallelSet {
    /// Ready tasks that may start now beside the running ones, best first.
    pub eligible: Vec<TaskId>,
    pub excluded: Vec<ParallelExcluded>,
    /// Tasks already running (active or verifying).
    pub running: Vec<TaskId>,
    /// Helpers allowed at once.
    pub max: u32,
}
