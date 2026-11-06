//! Stopping this chat's task helpers and recording why. Pausing the plan
//! and holding a task soft-stop the helpers concerned, closing the chat
//! and moving the plan soft-stop all of them: each ends at its next round
//! boundary and its task goes back to Pending with a resume pointer (its
//! kept worktree, when its change is in one). Stopping or abandoning the
//! plan cancels them at once, and the kernel's Stop has already put their
//! tasks back to Pending without a pointer.

use z_engine_protocol::work::{Actor, ResumePointer, WorkEdit};
use z_engine_protocol::{AgentInfo, AgentStatus, TaskId, WorktreeState};
use z_engine_work::{Proposal, Submission};

use super::bound::{CLOSED, HELD, Halt, PAUSED};
use super::facts::instant;
use super::helper::ended;
use super::lifecycle::report;
use super::plan::PlanHandle;
use crate::session::SessionCore;

/// What an accepted Plan-tab edit stops: every helper of the plan, or the
/// one on the task it names.
pub(crate) fn halt_for(edit: &WorkEdit) -> Option<(Halt, Option<TaskId>)> {
    match edit {
        WorkEdit::Pause => Some((Halt::Interrupt(PAUSED), None)),
        WorkEdit::HoldTask {
            task_id,
            held: true,
        } => Some((Halt::Interrupt(HELD), Some(task_id.clone()))),
        WorkEdit::Stop | WorkEdit::Abandon => Some((Halt::Stopped, None)),
        _ => None,
    }
}

/// Stops what `halt_for` picked on `plan`.
pub(crate) fn halt(core: &SessionCore, plan: &PlanHandle, (halt, task): (Halt, Option<TaskId>)) {
    core.work.helpers().halt(&plan.id, halt, |bound| {
        task.as_ref().is_none_or(|id| id == bound)
    });
}

/// The chat is closing: its helpers' tasks wait for them.
pub(crate) fn closing(core: &SessionCore) {
    core.work.helpers().halt_all(CLOSED);
}

/// A task-bound helper's run ended: `None` when it was not stopped (or
/// finished anyway), so its report and change are recorded as usual;
/// otherwise its task already waits (with a resume pointer, unless the plan
/// was stopped) and `Some(true)` asks to tell the coordinator.
pub(crate) fn settle_halted(core: &SessionCore, info: &AgentInfo, task: &TaskId) -> Option<bool> {
    let bound = core.work.helpers().remove(&info.agent_id)?;
    match bound.halt? {
        Halt::Stopped => {
            core.work.take_report(&info.agent_id, task);
            Some(false)
        }
        Halt::Interrupt(_) if info.status == AgentStatus::Completed => None,
        Halt::Interrupt(note) => {
            interrupt(core, &bound.plan, info, task, note);
            Some(note == PAUSED || note == HELD)
        }
    }
}

/// `info`'s helper stopped before finishing `task`.
fn interrupt(core: &SessionCore, plan: &PlanHandle, info: &AgentInfo, task: &TaskId, note: &str) {
    let worktree = info
        .worktree
        .as_ref()
        .filter(|tree| {
            matches!(
                tree.state,
                WorktreeState::Pending | WorktreeState::Conflicted
            )
        })
        .map(|tree| tree.path.clone());
    let resume = ResumePointer {
        session_id: core.id.clone(),
        agent_id: info.agent_id.clone(),
        worktree,
        note: note.to_string(),
    };
    let proposals = vec![Proposal::InterruptTask {
        task_id: task.clone(),
        resume,
    }];
    let result = plan.submit(&instant(core), |_| {
        Ok(Submission::new(Actor::Kernel, ended(), proposals))
    });
    report(core, result, "The helper's interruption was not recorded");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pausing_holding_and_stopping_pick_their_helpers() {
        let task = TaskId("t1".into());
        assert_eq!(
            halt_for(&WorkEdit::Pause),
            Some((Halt::Interrupt(PAUSED), None))
        );
        let hold = WorkEdit::HoldTask {
            task_id: task.clone(),
            held: true,
        };
        assert_eq!(
            halt_for(&hold),
            Some((Halt::Interrupt(HELD), Some(task.clone())))
        );
        let release = WorkEdit::ReleaseTask { task_id: task };
        assert_eq!(halt_for(&release), None);
        assert_eq!(halt_for(&WorkEdit::Stop), Some((Halt::Stopped, None)));
        assert_eq!(halt_for(&WorkEdit::Abandon), Some((Halt::Stopped, None)));
        assert_eq!(halt_for(&WorkEdit::Resume), None);
    }
}
