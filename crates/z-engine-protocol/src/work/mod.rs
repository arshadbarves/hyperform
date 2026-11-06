//! Work plans: persistent, dependency-aware plans of one project. The
//! kernel (`z-engine-work`) validates every change; these are the values it
//! exchanges with the engine, the store and the GUI.

pub mod audit;
pub mod edit;
pub mod finding;
pub mod health;
pub mod ledger;
pub mod packet;
pub mod parallel;
pub mod plan;
pub mod record;
pub mod report;
pub mod run;
pub mod task;
pub mod view;

pub use audit::{AuditFinding, AuditKind, AuditRef, AuditSeverity, AuditSource, AuditStatus};
pub use edit::{CriterionDraft, WorkEdit};
pub use finding::ReconcileFinding;
pub use health::{HealthLevel, PlanHealth};
pub use ledger::{
    ChangeCause, DesignDecision, Discovery, Entry, EntryBody, EntryStatus, EvidenceEntry,
    EvidenceRef, Experiment, ExperimentOutcome, PlanChange,
};
pub use packet::{
    PacketBlocker, PacketDependency, PacketEntry, PacketEvidence, PacketReason, PacketTaskLine,
    TaskPacket,
};
pub use parallel::{ParallelBlock, ParallelExcluded, ParallelSet};
pub use plan::{
    ParallelPolicy, PlanDepth, PlanHeader, PlanItem, PlanItemKind, PlanItemStatus, PlanStatus,
    PlanSummary, Provenance,
};
pub use record::{Actor, TaskPatch, WorkCause, WorkEvent, WorkRecord};
pub use report::{ReportClaims, ReportConfidence, ReportDraft, ReportSource, TaskReport};
pub use run::{Integration, TaskDispatch, TaskRun};
pub use task::{
    BlockReason, Criterion, CriterionMethod, CriterionStatus, ResumePointer, Task, TaskKind,
    TaskNotes, TaskOwner, TaskPriority, TaskScope, TaskStatus,
};
pub use view::{WhyStep, WorkSnapshot, WorkUpdate};

#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod tests;
