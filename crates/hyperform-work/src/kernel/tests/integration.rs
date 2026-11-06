use hyperform_protocol::work::{
    Actor, BlockReason, Integration, ParallelBlock, TaskStatus, WorkEdit, WorkEvent,
};

use super::run::{bound, running};
use crate::testkit::*;
use crate::{Proposal, Reason};

fn integrate(id: &str, integration: Integration) -> Proposal {
    Proposal::Integrate {
        task_id: tid(id),
        integration,
    }
}

#[test]
fn an_applied_change_moves_the_task_to_verifying() {
    let mut state = running();
    let pending = Integration::PendingApply {
        worktree: "/wt/a".into(),
    };
    let rejection = refuse(&state, main_agent(), vec![integrate("a", pending.clone())]);
    assert!(matches!(rejection.reasons[0], Reason::EngineOnly(_)));
    submit(
        &mut state,
        Actor::Kernel,
        vec![integrate("a", pending.clone())],
    );
    let a = state.task(&tid("a")).unwrap();
    assert_eq!(
        (a.status.clone(), a.run.integration.clone()),
        (TaskStatus::Active, Some(pending))
    );
    let applied = Integration::Applied {
        files: vec!["src/a/retry.rs".into()],
    };
    submit(&mut state, Actor::Kernel, vec![integrate("a", applied)]);
    let a = state.task(&tid("a")).unwrap();
    assert_eq!(
        (a.status.clone(), a.owner.clone()),
        (TaskStatus::Verifying, None)
    );
}

#[test]
fn a_conflict_keeps_the_worktree_and_a_serial_retry_runs_alone() {
    let mut state = running();
    let conflict = Integration::Conflict {
        paths: vec!["src/a/retry.rs".into()],
        worktree: "/wt/a".into(),
    };
    submit(
        &mut state,
        Actor::Kernel,
        vec![integrate("a", conflict.clone())],
    );
    let a = state.task(&tid("a")).unwrap();
    let detail = "applying the change conflicts in src/a/retry.rs".to_string();
    assert_eq!(
        a.status,
        TaskStatus::Blocked {
            reason: BlockReason::MergeConflict { detail }
        }
    );
    assert_eq!(
        (a.owner.clone(), a.run.integration.clone()),
        (None, Some(conflict))
    );
    let retry = Proposal::from(WorkEdit::RetrySerially { task_id: tid("a") });
    let records = submit(&mut state, user(), vec![retry]);
    assert!(records.iter().any(|record| matches!(
        record.event,
        WorkEvent::TaskRetried {
            serial: true,
            fresh_attempts: true,
            ..
        }
    )));
    let a = state.task(&tid("a")).unwrap();
    assert_eq!(a.status, TaskStatus::Pending);
    assert!(a.run.serial && a.run.integration.is_none() && a.run.dispatch.is_none());
    assert_eq!(a.run.forgiven_attempts, 1);
    let set = crate::parallel::eligible(&state, &parallel_facts(3));
    assert_eq!(set.eligible, [tid("a")]);
    assert_eq!(
        set.excluded[0].block,
        ParallelBlock::RunAlone { task_id: tid("a") }
    );
}

#[test]
fn a_kept_worktree_applied_after_its_conflict_verifies_the_task() {
    let mut state = running();
    let conflict = Integration::Conflict {
        paths: vec!["src/a/retry.rs".into()],
        worktree: "/wt/a".into(),
    };
    submit(&mut state, Actor::Kernel, vec![integrate("a", conflict)]);
    let applied = Integration::Applied {
        files: vec!["src/a/retry.rs".into()],
    };
    submit(&mut state, Actor::Kernel, vec![integrate("a", applied)]);
    assert_eq!(state.task(&tid("a")).unwrap().status, TaskStatus::Verifying);
}

#[test]
fn discarding_a_change_blocks_the_task() {
    let mut state = running();
    submit(
        &mut state,
        Actor::Kernel,
        vec![integrate("a", Integration::Discarded)],
    );
    let a = state.task(&tid("a")).unwrap();
    let discarded = TaskStatus::Blocked {
        reason: BlockReason::ChangeDiscarded,
    };
    assert_eq!((a.status.clone(), a.owner.clone()), (discarded, None));
    assert_eq!(a.run.integration, Some(Integration::Discarded));
    let retry = Proposal::from(WorkEdit::RetryTask { task_id: tid("a") });
    submit(&mut state, user(), vec![retry]);
    assert_eq!(state.task(&tid("a")).unwrap().status, TaskStatus::Pending);
}

#[test]
fn a_worktree_helper_cannot_complete_before_its_change_is_applied() {
    let mut state = running();
    let done = Proposal::SetTaskStatus {
        task_id: tid("a"),
        status: TaskStatus::Done,
    };
    let rejection = refuse_with(&state, worker("w1"), vec![done], &bound("a"));
    assert!(
        rejection.to_string().contains("integrates it"),
        "{rejection}"
    );
    let applied = Integration::Applied { files: vec![] };
    submit(&mut state, Actor::Kernel, vec![integrate("a", applied)]);
    assert_eq!(state.task(&tid("a")).unwrap().status, TaskStatus::Verifying);
}

#[test]
fn malformed_or_misplaced_integrations_are_refused() {
    let state = running();
    let bad = [
        Integration::PendingApply {
            worktree: " ".into(),
        },
        Integration::Conflict {
            paths: vec![],
            worktree: "/wt/a".into(),
        },
    ];
    for integration in bad {
        assert!(matches!(
            refuse(&state, Actor::Kernel, vec![integrate("a", integration)]).reasons[0],
            Reason::Invalid(_)
        ));
    }
    let applied = Integration::Applied { files: vec![] };
    let rejection = refuse(&state, Actor::Kernel, vec![integrate("b", applied)]);
    assert!(
        rejection
            .to_string()
            .contains("only a running task's change")
    );
}
