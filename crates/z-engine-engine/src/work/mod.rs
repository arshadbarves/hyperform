//! Work plans (`work_plans`, Experimental) in the engine: the process-wide
//! plan hub (one writer per plan: append, then apply), the session's
//! attachment (one chat drives a plan at a time; opening or attaching it
//! resumes it), and the seams that feed the model: tools, packets,
//! check evidence, compaction and reconciliation; task helpers (dispatch,
//! report, integration, interruption, the coordinator's reminders), the
//! scope-drift gate and the plan audit. The kernel in `z-engine-work`
//! decides every change.

mod attach;
mod audit;
mod bound;
mod checkpoint;
mod compaction;
mod coordinate;
mod dispatch;
mod drift;
mod driver;
mod evidence;
mod facts;
mod findings;
mod helper;
mod hub;
mod ids;
mod inject;
mod interrupt;
mod kept;
mod lifecycle;
mod list;
mod merged;
mod plan;
pub(crate) mod proposals;
mod reconcile;
mod replan;
mod resume;
mod session;

pub(crate) use attach::{attach_command, detach_command, ensure, reattach};
pub(crate) use audit::{AUDIT_AGENT, review_deep_plan};
pub(crate) use checkpoint::link as link_checkpoint;
pub(crate) use compaction::{CompactWork, split_discoveries};
pub(crate) use coordinate::{News, tell_coordinator};
pub(crate) use dispatch::{Admission, admit};
pub(crate) use drift::outside_scope;
pub(crate) use evidence::{link_check, record_steps};
pub(crate) use facts::{agent, current, enabled, instant, task_id};
pub(crate) use helper::{checks_of, conclude, integrate, save_report};
pub(crate) use hub::PlanHub;
pub(crate) use ids::Ids;
pub(crate) use inject::{due_packet, focus_files, helper_packet, pin};
pub(crate) use interrupt::{closing, settle_halted};
pub(crate) use kept::{foreign_apply, kept_discarded};
pub(crate) use lifecycle::{approve, attached, edit, planning_on};
pub(crate) use list::list_text;
pub(crate) use merged::verify_merged;
pub(crate) use plan::{PlanHandle, Submitted};
pub(crate) use reconcile::{after_records, at_turn_start, baseline};
pub(crate) use replan::{Replan, replan};
pub(crate) use session::WorkSession;
