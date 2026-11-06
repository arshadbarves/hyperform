//! Compaction planning: microcompaction clears old tool output in place;
//! summary compaction replaces older history with one summary message; the
//! task view sets aside earlier exchanges a new task does not need.
//! The engine performs the I/O (artifact spills, the summarizer request).

mod exchanges;
mod keeps;
mod micro;
mod ranked;
mod summary;
mod task_view;
mod touch;
mod transcript;

pub use exchanges::{Exchange, exchanges, relative};
pub use keeps::{KeepReason, protected_results};
pub use micro::{CLEARED_PREFIX, ClearTarget, apply_microcompact, plan_microcompact};
pub use ranked::plan_microcompact_ranked;
pub use summary::{SummaryPlan, apply_summary, is_summary, plan_summary, summary_message};
pub use task_view::{
    MAX_VIEW_PERCENT, MIN_HISTORY_TOKENS, TaskKeep, TaskViewPlan, apply_task_view, index_line,
    is_task_view_index, plan_task_view, task_keeps, task_view_index, worth_rebuilding,
};
pub use touch::{
    call_command, call_paths, is_check, is_edit, latest_request, named_paths, names_path,
    request_text,
};
pub use transcript::render_for_summary;
