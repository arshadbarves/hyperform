//! The task packet: what a model is given about its task, ranked and cut
//! to a token budget. The Context tab shows it, with why each entry is in.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{PlanId, TaskId};
use crate::work::ledger::{Entry, EvidenceRef};
use crate::work::plan::{PlanItem, PlanStatus};
use crate::work::task::{BlockReason, ResumePointer, Task, TaskStatus};

/// Why a ledger entry is in the packet, most relevant first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PacketReason {
    /// Linked to the task itself.
    Task,
    /// Linked to a parent (or further ancestor) of the task.
    Ancestor,
    /// Linked to one of the task's dependencies.
    Dependency,
    /// About files in the task's scope.
    Scope,
    /// Recent, with nothing more specific.
    Recent,
}

/// A one-line reference to another task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PacketTaskLine {
    pub id: TaskId,
    pub title: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PacketDependency {
    pub id: TaskId,
    pub title: String,
    pub status: TaskStatus,
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PacketEntry {
    pub entry: Entry,
    pub reason: PacketReason,
    #[ts(type = "number")]
    pub tokens: u64,
}

/// Something that keeps the task from moving on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum PacketBlocker {
    Blocked {
        reason: BlockReason,
    },
    /// A dependency that is not Done yet.
    Waiting {
        task: PacketTaskLine,
    },
    /// Held by the user.
    Held,
    /// The plan is not Active.
    Plan {
        status: PlanStatus,
    },
}

/// The latest evidence of one criterion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PacketEvidence {
    pub criterion_id: String,
    pub evidence: EvidenceRef,
    /// A check that ran on a different tree than the current one.
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskPacket {
    pub plan_id: PlanId,
    #[ts(type = "number")]
    pub revision: u64,
    pub goal: String,
    /// Active invariants; never dropped.
    pub invariants: Vec<PlanItem>,
    /// Active requirements the task serves.
    pub requirements: Vec<PlanItem>,
    /// The task with its criteria and steps; never dropped.
    pub task: Task,
    pub parent: Option<PacketTaskLine>,
    pub dependencies: Vec<PacketDependency>,
    pub entries: Vec<PacketEntry>,
    pub evidence: Vec<PacketEvidence>,
    pub blockers: Vec<PacketBlocker>,
    pub scope: Vec<String>,
    pub resume: Option<ResumePointer>,
    /// The next ready task other than this one.
    pub next_ready: Option<PacketTaskLine>,
    /// Estimated size of what was kept.
    #[ts(type = "number")]
    pub tokens: u64,
    #[ts(type = "number")]
    pub budget: u64,
    /// Ledger entries, dependencies or evidence left out to fit the budget.
    pub dropped: u32,
}
