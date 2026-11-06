//! Pure prompt assembly: layered system prompt, reminders, repo map, token
//! estimates and compaction planning. No filesystem, process or network
//! I/O: callers pass file contents in.

pub mod breakdown;
pub mod cache;
pub mod compaction;
pub mod environment;
pub mod instructions;
pub mod reminders;
pub mod repo_map;
pub mod sections;
pub mod system;
pub mod template;
mod text;
pub mod tokens;
pub mod wellformed;
pub mod work;

pub use breakdown::context_breakdown;
pub use cache::{message_breakpoints, system_breakpoint};
pub use compaction::{
    CLEARED_PREFIX, ClearTarget, SummaryPlan, apply_microcompact, apply_summary, plan_microcompact,
    plan_summary, render_for_summary, summary_message,
};
pub use environment::{Environment, GitInfo, render_environment, today};
pub use instructions::{InstructionDoc, render_instructions};
pub use reminders::{
    auto_check_failed, files_changed_externally, interrupted, job_finished, nested_instructions,
    plan_approved, plan_mode_active, steering, todo_nudge, todo_state, verification_required,
    wrap_reminder,
};
pub use repo_map::{SourceFile, repo_map, repo_map_ranked, supported_extension};
pub use sections::PromptSection;
pub use system::{SystemInputs, build_system};
pub use template::render_template;
pub use tokens::{
    MEDIA_TOKENS, estimate_block, estimate_message, estimate_messages, estimate_text,
    estimate_tools,
};
pub use wellformed::{MISSING_RESULT, merge_adjacent_roles, well_formed};
pub use work::{
    HelperNews, ResumeBrief, TaskHealth, TaskListView, WorkPin, audit_request, helper_brief,
    packet_text, task_line, task_list, task_status, work_coordinate, work_finding_added,
    work_packet, work_pin, work_reconcile, work_resume,
};
