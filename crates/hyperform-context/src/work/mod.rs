//! Work-plan text the model reads: the task packet (`TaskGet` and the
//! packet reminder), the turn-start pin, reconciliation findings, the task
//! list, parallel work (helper briefs, coordinator reminders, the audit
//! request) and the resume reminder. Statuses use the words of
//! `TaskUpdate` (`in_progress`, `completed`), so what the model reads is
//! what it writes.

mod coordinate;
mod packet;
mod phrases;
mod resume;
mod views;

pub use coordinate::{
    HelperNews, audit_request, helper_brief, work_coordinate, work_finding_added,
};
pub use packet::{packet_text, task_line, task_status, work_packet};
pub use resume::{ResumeBrief, work_resume};
pub use views::{
    TaskHealth, TaskListView, WorkPin, finding_lines, task_list, work_pin, work_reconcile,
};

#[cfg(test)]
mod tests;
