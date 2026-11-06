//! What resuming a plan finds when a chat opens with it (after an app
//! restart too) or attaches it: Active tasks whose helper is gone. Each
//! becomes Pending with a resume pointer (`Proposal::InterruptTask`); the
//! main agent of the opening chat keeps its own task, and so do this
//! chat's helpers that still run or whose change still waits at its apply
//! card, and the helpers of the chat the plan moved from that are still
//! stopping. What resuming reports counts every task interrupted since the
//! last resume, so the ones a closing chat already stopped count too.

use hyperform_protocol::work::{
    ChangeCause, EntryBody, Integration, ResumePointer, Task, TaskStatus, WorkEvent, WorkRecord,
};
use hyperform_protocol::{AgentId, SessionId, TaskId};

use crate::graph::is_closed;
use crate::state::PlanState;

/// The opening chat and its helpers still at work.
#[derive(Debug, Clone, Copy)]
pub struct Alive<'a> {
    pub session: &'a SessionId,
    /// This chat's helpers that still run.
    pub running: &'a [AgentId],
    /// This chat's helpers whose worktree change waits at its apply card.
    pub waiting: &'a [AgentId],
    /// Helpers of the live chat the plan just moved from that are still
    /// stopping: each records its own interruption.
    pub stopping: &'a [(SessionId, AgentId)],
}

/// The note of a task whose helper stopped without finishing.
pub const STOPPED: &str = "its helper stopped before it finished";
/// The note of a task whose change no longer waits at an apply card here.
pub const UNAPPLIED: &str = "its helper finished, but its change was never applied";
/// The note of a task the main agent of another chat had started.
pub const OTHER_MAIN: &str = "the main agent of another chat had started it";

/// The tasks to interrupt, each with where it can pick up.
pub fn interrupted(state: &PlanState, alive: &Alive) -> Vec<(TaskId, ResumePointer)> {
    state
        .tasks
        .values()
        .filter(|task| task.status == TaskStatus::Active)
        .filter_map(|task| pointer(task, alive).map(|pointer| (task.id.clone(), pointer)))
        .collect()
}

/// Open tasks interrupted after the plan's last resume (a `PlanChange`
/// with cause `resume`), or ever when it has none, that still wait with
/// their resume pointer: whether a closing chat stopped them or resuming
/// found them now.
pub fn interrupted_since_resume(records: &[WorkRecord], state: &PlanState) -> Vec<TaskId> {
    let since = records
        .iter()
        .rposition(|record| match &record.event {
            WorkEvent::EntryRecorded { entry } => matches!(
                &entry.body,
                EntryBody::PlanChange(change) if change.cause == ChangeCause::Resume
            ),
            _ => false,
        })
        .map_or(0, |at| at + 1);
    let mut tasks: Vec<TaskId> = records[since..]
        .iter()
        .filter_map(|record| match &record.event {
            WorkEvent::TaskInterrupted { task_id, .. } => Some(task_id),
            _ => None,
        })
        .filter(|id| {
            state
                .task(id)
                .is_some_and(|task| task.resume.is_some() && !is_closed(&task.status))
        })
        .cloned()
        .collect();
    tasks.sort();
    tasks.dedup();
    tasks
}

fn pointer(task: &Task, alive: &Alive) -> Option<ResumePointer> {
    let owner = task.owner.as_ref()?;
    let here = owner.session_id == *alive.session;
    let waiting = matches!(task.run.integration, Some(Integration::PendingApply { .. }));
    let stopping = alive
        .stopping
        .iter()
        .any(|(session, agent)| *session == owner.session_id && *agent == owner.agent_id);
    let note = if owner.agent_id.is_main() {
        if here {
            return None;
        }
        OTHER_MAIN
    } else if stopping || (here && alive.running.contains(&owner.agent_id)) {
        return None;
    } else if waiting {
        if here && alive.waiting.contains(&owner.agent_id) {
            return None;
        }
        UNAPPLIED
    } else {
        STOPPED
    };
    let worktree = match (&task.run.integration, &task.run.dispatch) {
        (Some(Integration::PendingApply { worktree }), _) => Some(worktree.clone()),
        (_, Some(dispatch)) if dispatch.agent_id == owner.agent_id => dispatch.worktree.clone(),
        _ => None,
    };
    Some(ResumePointer {
        session_id: owner.session_id.clone(),
        agent_id: owner.agent_id.clone(),
        worktree,
        note: note.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use hyperform_protocol::work::{
        Actor, PlanChange, PlanStatus, TaskDispatch, TaskOwner, WorkCause,
    };

    use super::*;
    use crate::decide;
    use crate::proposal::{EntryDraft, Proposal, Submission};
    use crate::testkit::{add, facts, main_agent, plan, set, submit, tid, user};

    fn dispatched(state: &mut PlanState, id: &str, agent: &str) {
        let dispatch = TaskDispatch {
            session_id: SessionId::from("S"),
            agent_id: AgentId::from(agent),
            worktree: Some(format!("/wt/{agent}")),
            attempt: 0,
            at: 0,
        };
        let proposal = Proposal::DispatchTask {
            task_id: tid(id),
            dispatch,
        };
        submit(state, main_agent(), vec![proposal]);
    }

    #[test]
    fn gone_helpers_and_other_chats_leave_resume_pointers() {
        let mut state = plan(PlanStatus::Active);
        for id in ["a", "b", "c"] {
            add(&mut state, id, &[]);
        }
        dispatched(&mut state, "a", "agt_a");
        submit(&mut state, main_agent(), vec![set("b", TaskStatus::Active)]);
        let here = SessionId::from("S");
        let none: [AgentId; 0] = [];
        let alive = Alive {
            session: &here,
            running: &none,
            waiting: &none,
            stopping: &[],
        };
        let found = interrupted(&state, &alive);
        assert_eq!(found.len(), 1, "the main agent of this chat keeps b");
        let (task, pointer) = &found[0];
        assert_eq!(task, &tid("a"));
        assert_eq!(pointer.agent_id, AgentId::from("agt_a"));
        assert_eq!(pointer.worktree.as_deref(), Some("/wt/agt_a"));
        assert_eq!(pointer.note, STOPPED);

        let running = [AgentId::from("agt_a")];
        let alive = Alive {
            running: &running,
            ..alive
        };
        assert!(interrupted(&state, &alive).is_empty(), "a still runs here");

        let other = SessionId::from("T");
        let elsewhere = Alive {
            session: &other,
            running: &none,
            waiting: &none,
            stopping: &[],
        };
        let notes = |alive: &Alive| -> Vec<String> {
            let found = interrupted(&state, alive).into_iter();
            found.map(|(_, pointer)| pointer.note).collect()
        };
        assert_eq!(
            notes(&elsewhere),
            vec![STOPPED.to_string(), OTHER_MAIN.to_string()]
        );
        let stopping = [(here.clone(), AgentId::from("agt_a"))];
        let moved = Alive {
            stopping: &stopping,
            ..elsewhere
        };
        assert_eq!(
            notes(&moved),
            vec![OTHER_MAIN.to_string()],
            "a's helper, stopping in the chat the plan moved from, records it"
        );
    }

    #[test]
    fn a_change_waiting_at_this_chats_apply_card_keeps_waiting() {
        let mut state = plan(PlanStatus::Active);
        add(&mut state, "a", &[]);
        dispatched(&mut state, "a", "agt_a");
        let pending = Proposal::Integrate {
            task_id: tid("a"),
            integration: Integration::PendingApply {
                worktree: "/wt/agt_a".into(),
            },
        };
        submit(&mut state, Actor::Kernel, vec![pending]);
        let here = SessionId::from("S");
        let waiting = [AgentId::from("agt_a")];
        let alive = Alive {
            session: &here,
            running: &[],
            waiting: &waiting,
            stopping: &[],
        };
        assert!(interrupted(&state, &alive).is_empty());
        let gone = Alive {
            waiting: &[],
            ..alive
        };
        let found = interrupted(&state, &gone);
        assert_eq!(found[0].1.note, UNAPPLIED);
        let owner = TaskOwner {
            session_id: here.clone(),
            agent_id: AgentId::from("agt_a"),
        };
        assert_eq!(state.task(&tid("a")).unwrap().owner, Some(owner));
    }

    fn stop(state: &mut PlanState, id: &str, agent: &str) -> Vec<WorkRecord> {
        let resume = ResumePointer {
            session_id: SessionId::from("S"),
            agent_id: AgentId::from(agent),
            worktree: None,
            note: STOPPED.into(),
        };
        let proposal = Proposal::InterruptTask {
            task_id: tid(id),
            resume,
        };
        submit(state, Actor::Kernel, vec![proposal])
    }

    fn resumed(state: &mut PlanState) -> Vec<WorkRecord> {
        let change = PlanChange {
            cause: ChangeCause::Resume,
            source: None,
            original: "revision 3".into(),
            new: String::new(),
            reason: "Resumed: 1 task was interrupted".into(),
            evidence: Vec::new(),
            affected: Vec::new(),
        };
        let draft = EntryDraft::new(EntryBody::PlanChange(change));
        let submission = Submission::new(
            Actor::Kernel,
            WorkCause::Resume,
            vec![Proposal::Record(Box::new(draft))],
        );
        let records = decide(state, &submission, &facts()).expect("recorded");
        state.apply_all(&records);
        records
    }

    #[test]
    fn resuming_counts_tasks_interrupted_since_the_last_resume() {
        let mut state = plan(PlanStatus::Active);
        add(&mut state, "a", &[]);
        add(&mut state, "b", &[]);
        dispatched(&mut state, "a", "agt_a");
        let mut log = stop(&mut state, "a", "agt_a");
        assert_eq!(
            interrupted_since_resume(&log, &state),
            vec![tid("a")],
            "stopped when the chat closed"
        );
        log.extend(resumed(&mut state));
        assert!(interrupted_since_resume(&log, &state).is_empty());
        dispatched(&mut state, "b", "agt_b");
        log.extend(stop(&mut state, "b", "agt_b"));
        assert_eq!(interrupted_since_resume(&log, &state), vec![tid("b")]);
        submit(&mut state, user(), vec![set("b", TaskStatus::Cancelled)]);
        assert!(
            interrupted_since_resume(&log, &state).is_empty(),
            "a closed task no longer waits"
        );
    }
}
