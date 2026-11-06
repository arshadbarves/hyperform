//! Folding a task's run: a dispatch starts a new run, the report and the
//! integration fill it in, an interruption leaves a resume pointer (whose
//! worktree a discard drops), and a retry clears it for the next one.

use z_engine_protocol::work::{Task, TaskReport, WorkEvent};

/// False when `event` is not a run event.
pub(super) fn apply(task: &mut Task, event: &WorkEvent) -> bool {
    let run = &mut task.run;
    match event {
        WorkEvent::TaskDispatched { dispatch, .. } => {
            run.dispatch = Some(dispatch.clone());
            run.integration = None;
            run.report = None;
            task.resume = None;
        }
        WorkEvent::TaskReported { report, .. } => run.report = Some(TaskReport::clone(report)),
        WorkEvent::TaskIntegrated { integration, .. } => {
            run.integration = Some(integration.clone());
        }
        WorkEvent::TaskInterrupted { resume, .. } => {
            run.integration = None;
            run.forgiven_attempts = (run.forgiven_attempts + 1).min(task.attempts);
            task.resume = Some(resume.clone());
        }
        WorkEvent::ResumeWorktreeDiscarded { .. } => {
            if let Some(resume) = task.resume.as_mut() {
                resume.worktree = None;
            }
        }
        WorkEvent::TaskRetried {
            serial,
            fresh_attempts,
            ..
        } => {
            run.dispatch = None;
            run.integration = None;
            run.serial = *serial;
            if *fresh_attempts {
                run.forgiven_attempts = task.attempts;
            }
        }
        _ => return false,
    }
    true
}
