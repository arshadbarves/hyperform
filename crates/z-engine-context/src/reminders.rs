//! `<system-reminder>` blocks the engine appends to user messages. Each
//! builder renders one template from [`z_engine_prompts::reminders`].

use z_engine_prompts::reminders as templates;
use z_engine_protocol::{
    JobInfo, JobKind, JobStatus, TodoItem, TodoStatus, VerificationMode, VerificationOutcome,
};

use crate::instructions::{InstructionDoc, render_docs};
use crate::template::render_template;
use crate::text::single_line;

/// Wraps `body` in `<system-reminder>` tags on their own lines.
pub fn wrap_reminder(body: &str) -> String {
    format!("<system-reminder>\n{}\n</system-reminder>", body.trim())
}

fn reminder(template: &str, values: &[(&str, &str)]) -> String {
    wrap_reminder(&render_template(template, values))
}

/// The todo list is empty: suggest `TodoWrite` for multi-step work.
pub fn todo_nudge() -> String {
    reminder(templates::TODO_NUDGE, &[])
}

/// The current todo list as `[x]`/`[~]`/`[ ]` lines.
pub fn todo_state(todos: &[TodoItem]) -> String {
    let lines: Vec<String> = todos
        .iter()
        .map(|todo| {
            format!(
                "{} {}",
                todo_marker(todo.status),
                single_line(&todo.content)
            )
        })
        .collect();
    reminder(templates::TODO_STATE, &[("todos", &lines.join("\n"))])
}

/// Plan mode: read-only research, then `ExitPlanMode` with the plan.
pub fn plan_mode_active() -> String {
    reminder(templates::PLAN_MODE_ACTIVE, &[])
}

/// The user approved `plan` (possibly edited); implementation may start.
pub fn plan_approved(plan: &str) -> String {
    reminder(templates::PLAN_APPROVED, &[("plan", plan.trim())])
}

/// Files modified on disk since the model last read them.
pub fn files_changed_externally(paths: &[String]) -> String {
    let list: Vec<String> = paths.iter().map(|path| format!("- {path}")).collect();
    reminder(
        templates::FILES_CHANGED_EXTERNALLY,
        &[("paths", &list.join("\n"))],
    )
}

/// Instruction files found in directories the model just accessed.
pub fn nested_instructions(docs: &[InstructionDoc]) -> String {
    let docs = render_docs(docs).unwrap_or_default();
    reminder(templates::NESTED_INSTRUCTIONS, &[("docs", &docs)])
}

/// A background shell or agent job finished; points at `JobOutput`.
pub fn job_finished(job: &JobInfo) -> String {
    let exit_code = job
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_default();
    reminder(
        templates::JOB_FINISHED,
        &[
            ("kind", job_kind_label(job.kind)),
            ("job_id", job.job_id.as_str()),
            ("label", &single_line(&job.label)),
            ("status", job_status_label(job.status)),
            ("exit_code", &exit_code),
        ],
    )
}

/// Messages the user sent while the model was working, as block quotes.
pub fn steering(messages: &[String]) -> String {
    let quoted: Vec<String> = messages.iter().map(|message| quote(message)).collect();
    reminder(templates::STEERING, &[("messages", &quoted.join("\n\n"))])
}

/// Changed files are unverified while `mode` requires verification.
pub fn verification_required(outcome: &VerificationOutcome, mode: VerificationMode) -> String {
    let reason = match outcome {
        VerificationOutcome::Unverified { reason } | VerificationOutcome::Failed { reason } => {
            reason.trim()
        }
        VerificationOutcome::NotApplicable | VerificationOutcome::Verified { .. } => "",
    };
    reminder(
        templates::VERIFICATION_REQUIRED,
        &[
            ("status", outcome.label()),
            ("reason", reason),
            ("mode", mode_label(mode)),
        ],
    )
}

/// Checks the harness ran at the stop boundary failed with `summary`.
pub fn auto_check_failed(summary: &str) -> String {
    reminder(templates::AUTO_CHECK_FAILED, &[("summary", summary.trim())])
}

/// The user interrupted the previous response.
pub fn interrupted() -> String {
    reminder(templates::INTERRUPTED, &[])
}

fn todo_marker(status: TodoStatus) -> &'static str {
    match status {
        TodoStatus::Completed => "[x]",
        TodoStatus::InProgress => "[~]",
        TodoStatus::Pending => "[ ]",
    }
}

fn job_kind_label(kind: JobKind) -> &'static str {
    match kind {
        JobKind::Shell => "shell command",
        JobKind::Agent => "agent",
    }
}

fn job_status_label(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Running => "running",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
        JobStatus::Killed => "killed",
    }
}

fn mode_label(mode: VerificationMode) -> &'static str {
    match mode {
        VerificationMode::Off => "off",
        VerificationMode::Report => "report",
        VerificationMode::Auto => "auto",
        VerificationMode::Strict => "strict",
    }
}

fn quote(message: &str) -> String {
    message
        .trim()
        .lines()
        .map(|line| {
            if line.is_empty() {
                ">".to_string()
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "reminders_tests.rs"]
mod tests;
