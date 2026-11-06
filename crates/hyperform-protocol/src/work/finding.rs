//! Deterministic reconciliation findings: facts about the workspace or the
//! graph that make the plan need another look.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::TaskId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ReconcileFinding {
    /// Files in the task's scope were deleted.
    ScopeDeleted {
        task_id: TaskId,
        files: Vec<String>,
    },
    /// A Done task's evidence no longer holds: files in its scope changed,
    /// or the engine judged its check records stale.
    StaleEvidence {
        task_id: TaskId,
        files: Vec<String>,
        records: Vec<String>,
    },
    DependencyFailed {
        task_id: TaskId,
        depends_on: TaskId,
    },
    DependencySuperseded {
        task_id: TaskId,
        depends_on: TaskId,
        by: TaskId,
    },
    /// Open tasks with no ordering between them declare overlapping scopes.
    Overlap {
        tasks: Vec<TaskId>,
        files: Vec<String>,
    },
    /// Open tasks with the same normalized title.
    Duplicate {
        tasks: Vec<TaskId>,
        title: String,
    },
}

impl ReconcileFinding {
    /// The tasks the finding is about.
    pub fn tasks(&self) -> Vec<&TaskId> {
        match self {
            Self::ScopeDeleted { task_id, .. } | Self::StaleEvidence { task_id, .. } => {
                vec![task_id]
            }
            Self::DependencyFailed {
                task_id,
                depends_on,
            }
            | Self::DependencySuperseded {
                task_id,
                depends_on,
                ..
            } => vec![task_id, depends_on],
            Self::Overlap { tasks, .. } | Self::Duplicate { tasks, .. } => tasks.iter().collect(),
        }
    }
}
