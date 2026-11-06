use z_engine_protocol::work::{Actor, PlanStatus, ResumePointer, TaskStatus, WorkEdit, WorkEvent};
use z_engine_protocol::{AgentId, SessionId};

use super::run::{bound, running, send};
use crate::testkit::*;
use crate::{Proposal, Submission, decide, dispatch};

fn pointer(agent: &str) -> ResumePointer {
    ResumePointer {
        session_id: SessionId::from("S"),
        agent_id: AgentId::from(agent),
        worktree: Some("/wt/a".into()),
        note: "its helper stopped before it finished".into(),
    }
}

fn interrupt(agent: &str) -> Proposal {
    Proposal::InterruptTask {
        task_id: tid("a"),
        resume: pointer(agent),
    }
}

#[test]
fn an_interrupted_task_waits_with_its_resume_pointer() {
    let mut state = running();
    let records = submit(&mut state, Actor::Kernel, vec![interrupt("w1")]);
    assert!(matches!(
        records[0].event,
        WorkEvent::TaskTransitioned { .. }
    ));
    assert!(matches!(records[1].event, WorkEvent::TaskReleased { .. }));
    assert!(matches!(
        records[2].event,
        WorkEvent::TaskInterrupted { .. }
    ));
    let a = state.task(&tid("a")).unwrap();
    assert_eq!(a.status, TaskStatus::Pending);
    assert_eq!(a.owner, None);
    assert_eq!(a.resume, Some(pointer("w1")));
    assert_eq!(dispatch::attempts_used(a), 0);

    submit_with(
        &mut state,
        main_agent(),
        vec![send("a", "w1", Some("/wt/a"))],
        &parallel_facts(3),
    );
    let a = state.task(&tid("a")).unwrap();
    assert_eq!(
        (a.status.clone(), a.resume.clone()),
        (TaskStatus::Active, None)
    );
}

#[test]
fn only_the_engine_interrupts_and_only_the_running_agent() {
    let state = running();
    for actor in [user(), main_agent(), worker("w1")] {
        let rejection = refuse(&state, actor, vec![interrupt("w1")]);
        let text = rejection.to_string();
        assert!(text.contains("interrupting a task's helper"), "{text}");
    }
    let mut other = state.clone();
    assert!(submit(&mut other, Actor::Kernel, vec![interrupt("w2")]).is_empty());
    assert_eq!(other, state);
}

#[test]
fn resuming_belongs_to_the_agent_in_the_pointer_of_this_chat() {
    let mut state = running();
    submit(&mut state, Actor::Kernel, vec![interrupt("w1")]);
    let a = state.task(&tid("a")).unwrap();
    let here = SessionId::from("S");
    assert!(dispatch::resume_check(a, &here, &AgentId::from("w1")).is_ok());
    let other = dispatch::resume_check(a, &here, &AgentId::from("w2"))
        .unwrap_err()
        .to_string();
    assert!(other.contains("only agent w1"), "{other}");
    let elsewhere = dispatch::resume_check(a, &SessionId::from("T"), &AgentId::from("w1"))
        .unwrap_err()
        .to_string();
    assert!(elsewhere.contains("another chat"), "{elsewhere}");
}

fn discard(agent: &str) -> Proposal {
    Proposal::DiscardResumeWorktree {
        task_id: tid("a"),
        agent_id: AgentId::from(agent),
    }
}

#[test]
fn discarding_the_kept_worktree_leaves_the_pointer_without_it() {
    let mut state = running();
    submit(&mut state, Actor::Kernel, vec![interrupt("w1")]);
    for actor in [user(), main_agent(), worker("w1")] {
        let text = refuse(&state, actor, vec![discard("w1")]).to_string();
        assert!(text.contains("resume pointer's worktree"), "{text}");
    }
    let mut other = state.clone();
    assert!(submit(&mut other, Actor::Kernel, vec![discard("w2")]).is_empty());
    assert_eq!(other, state, "the pointer names another agent");

    let records = submit(&mut state, Actor::Kernel, vec![discard("w1")]);
    assert!(matches!(
        records[0].event,
        WorkEvent::ResumeWorktreeDiscarded { .. }
    ));
    let a = state.task(&tid("a")).unwrap();
    let kept = ResumePointer {
        worktree: None,
        ..pointer("w1")
    };
    assert_eq!(
        (a.status.clone(), a.resume.clone()),
        (TaskStatus::Pending, Some(kept))
    );
    assert!(
        submit(&mut state, Actor::Kernel, vec![discard("w1")]).is_empty(),
        "nothing is left to drop"
    );
}

#[test]
fn a_held_task_is_released_by_the_release_edit() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let hold = Submission::edit(WorkEdit::HoldTask {
        task_id: tid("a"),
        held: true,
    });
    state.apply_all(&decide(&state, &hold, &facts()).unwrap());
    assert!(state.task(&tid("a")).unwrap().held);
    let release = Submission::edit(WorkEdit::ReleaseTask { task_id: tid("a") });
    state.apply_all(&decide(&state, &release, &facts()).unwrap());
    assert!(!state.task(&tid("a")).unwrap().held);
}

#[test]
fn an_active_plan_is_done_when_its_last_open_task_closes() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    assert_eq!(state.header.status, PlanStatus::Active);
    let records = submit(
        &mut state,
        main_agent(),
        vec![set("b", TaskStatus::Cancelled)],
    );
    let last = records.last().unwrap();
    assert_eq!(last.actor, Actor::Kernel);
    assert!(matches!(
        last.event,
        WorkEvent::PlanStatusChanged {
            to: PlanStatus::Done,
            ..
        }
    ));
    assert_eq!(state.header.status, PlanStatus::Done);
}

#[test]
fn a_reopened_plan_stays_active_until_a_task_closes_again() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    let reopen = Submission::edit(WorkEdit::Reopen);
    state.apply_all(&decide(&state, &reopen, &facts()).unwrap());
    assert_eq!(state.header.status, PlanStatus::Active);
    add(&mut state, "b", &[]);
    assert_eq!(state.header.status, PlanStatus::Active);

    let mut paused = plan(PlanStatus::Active);
    add(&mut paused, "a", &[]);
    let pause = Submission::edit(WorkEdit::Pause);
    paused.apply_all(&decide(&paused, &pause, &facts()).unwrap());
    submit(&mut paused, user(), vec![set("a", TaskStatus::Done)]);
    assert_eq!(paused.header.status, PlanStatus::Paused);
}

#[test]
fn a_helper_cannot_finish_its_task_once_it_was_interrupted() {
    let mut state = running();
    submit(&mut state, Actor::Kernel, vec![interrupt("w1")]);
    refuse_with(
        &state,
        worker("w1"),
        vec![set("a", TaskStatus::Done)],
        &bound("a"),
    );
}
