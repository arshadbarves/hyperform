//! Pure work-plan kernel: the task graph, the task and plan state machines,
//! folding a plan log into `PlanState`, validating proposals against the
//! authority rules and the completion gate, packet selection, parallel
//! eligibility and dispatch, helper reports, the plan audit and health,
//! deterministic reconciliation, the tasks a replan holds, what waits on
//! the user, and what resuming a plan finds interrupted. No filesystem,
//! process or network access:
//! the engine supplies `Facts` and persists the records the kernel returns.

pub mod audit;
pub mod authority;
pub mod depth;
pub mod dispatch;
mod error;
mod facts;
mod glob;
pub mod graph;
pub mod health;
pub mod held;
pub mod kernel;
pub mod needs;
pub mod packet;
pub mod parallel;
mod proposal;
pub mod reconcile;
pub mod report;
pub mod resume;
pub mod scope;
mod state;
pub mod transitions;
mod why;

#[cfg(test)]
mod testkit;

pub use audit::{AuditInput, RISKY_PATHS, audit};
pub use authority::Role;
pub use depth::{DepthSignals, assess, assess_plan};
pub use dispatch::DispatchCheck;
pub use error::{FoldError, PacketError, Reason, Rejection};
pub use facts::{AuditScope, Facts, Limits, RunFacts};
pub use graph::Readiness;
pub use health::health;
pub use held::replan_held;
pub use kernel::{create, decide, rejected};
pub use needs::needs_you;
pub use packet::{Estimate, estimate_chars, select};
pub use proposal::{EntryDraft, PlanDraft, Proposal, Submission, TaskDraft};
pub use reconcile::findings;
pub use state::{Changes, PlanState};
pub use why::why;
