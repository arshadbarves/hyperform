//! The resume reminder: the first turn after a chat opened with its plan
//! (after an app restart too) or attached it hears what was completed,
//! what is current (interrupted tasks with their resume pointers), the
//! known issues, the open reconciliation findings and the next step; the
//! task packet follows it.

use z_engine_prompts::reminders;
use z_engine_protocol::work::{EvidenceRef, PlanStatus, ReconcileFinding, Task, TaskStatus};

use super::packet::task_line;
use super::phrases::{evidence, finding, list, render};
use crate::reminders::wrap_reminder;
use crate::text::single_line;

/// Completed tasks listed at most (the most recently updated).
const COMPLETED_SHOWN: usize = 12;

/// What the resume reminder says about the plan.
#[derive(Debug, Clone, Copy)]
pub struct ResumeBrief<'a> {
    pub goal: &'a str,
    pub status: PlanStatus,
    /// What resuming found, such as `Resumed: 1 task was interrupted`.
    pub summary: &'a str,
    pub tasks: &'a [&'a Task],
    pub findings: &'a [ReconcileFinding],
    pub next: Option<&'a Task>,
}

pub fn work_resume(brief: &ResumeBrief) -> String {
    let mut done: Vec<&Task> = brief
        .tasks
        .iter()
        .copied()
        .filter(|task| task.status == TaskStatus::Done)
        .collect();
    done.sort_by_key(|task| std::cmp::Reverse(task.updated_at));
    let hidden = done.len().saturating_sub(COMPLETED_SHOWN);
    let mut completed = list(
        done.iter()
            .take(COMPLETED_SHOWN)
            .map(|task| completed_line(task)),
    );
    if hidden > 0 {
        completed.push_str(&format!("\n- and {hidden} more"));
    }
    let current = brief.tasks.iter().filter(|task| is_current(task));
    let issues = brief.tasks.iter().filter(|task| {
        matches!(
            task.status,
            TaskStatus::Blocked { .. } | TaskStatus::Failed { .. }
        )
    });
    let values = [
        ("goal", single_line(brief.goal)),
        ("status", brief.status.label().to_string()),
        ("summary", single_line(brief.summary)),
        ("completed", completed),
        ("current", list(current.map(|task| current_line(task)))),
        ("issues", list(issues.map(|task| task_line(task)))),
        ("findings", list(brief.findings.iter().map(finding))),
        ("next", brief.next.map(task_line).unwrap_or_default()),
    ];
    wrap_reminder(&render(reminders::WORK_RESUME, &values))
}

fn is_current(task: &Task) -> bool {
    matches!(task.status, TaskStatus::Active | TaskStatus::Verifying)
        || (task.status == TaskStatus::Pending && task.resume.is_some())
}

/// `t1: Title (completed); evidence: ...` with the latest evidence.
fn completed_line(task: &Task) -> String {
    let latest = task
        .acceptance
        .iter()
        .filter_map(|criterion| criterion.evidence.last())
        .max_by_key(|evidence| evidence_at(evidence));
    match latest {
        Some(latest) => format!("{}; evidence: {}", task_line(task), evidence(latest)),
        None => task_line(task),
    }
}

fn evidence_at(evidence: &EvidenceRef) -> u64 {
    match evidence {
        EvidenceRef::Check { at, .. } => *at,
        _ => 0,
    }
}

/// `t2: Title (pending); interrupted: <note>; resume agent agt_1 in <worktree>`.
fn current_line(task: &Task) -> String {
    let Some(resume) = &task.resume else {
        return task_line(task);
    };
    let place = resume
        .worktree
        .as_deref()
        .map(|worktree| format!(" in {worktree}"))
        .unwrap_or_default();
    format!(
        "{}; interrupted: {}; resume agent {}{place}",
        task_line(task),
        single_line(&resume.note),
        resume.agent_id
    )
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::work::ResumePointer;
    use z_engine_protocol::{AgentId, SessionId, TaskId};

    use super::super::tests::task;
    use super::*;

    #[test]
    fn the_resume_brief_lists_where_the_work_stands() {
        let done = task("t1", "Parse", TaskStatus::Done);
        let mut interrupted = task("t2", "Retry", TaskStatus::Pending);
        interrupted.resume = Some(ResumePointer {
            session_id: SessionId::from("ses_1"),
            agent_id: AgentId::from("agt_1"),
            worktree: Some("/wt/agt_1".into()),
            note: "the chat closed".into(),
        });
        let failed = task(
            "t3",
            "Docs",
            TaskStatus::Failed {
                reason: "no access".into(),
            },
        );
        let waiting = task("t4", "Ship", TaskStatus::Pending);
        let tasks = [&done, &interrupted, &failed, &waiting];
        let findings = [ReconcileFinding::ScopeDeleted {
            task_id: TaskId::from("t4"),
            files: vec!["src/gone.rs".into()],
        }];
        let text = work_resume(&ResumeBrief {
            goal: "Ship retries",
            status: PlanStatus::Active,
            summary: "Resumed: 1 task was interrupted",
            tasks: &tasks,
            findings: &findings,
            next: Some(&waiting),
        });
        assert!(text.contains("\"Ship retries\" again; the plan is active. Resumed: 1 task"));
        assert!(
            text.contains("Completed:\n- t1: Parse (completed)"),
            "{text}"
        );
        assert!(text.contains(
            "Current:\n- t2: Retry (pending); interrupted: the chat closed; resume agent agt_1 in /wt/agt_1"
        ));
        assert!(text.contains("Known issues:\n- t3: Docs (failed: no access)"));
        assert!(text.contains("findings:\n- t4: files in its scope were deleted"));
        assert!(text.contains("Next: t4: Ship (pending)"));
        assert!(!text.contains("{{"));
    }
}
