//! Follow-ups the kernel derives after a submission: a task that failed or
//! was superseded blocks its dependents, and they unblock once it is Done
//! (or no longer needed) or the dependency is removed. Reopening a Done
//! task whose scope or criteria changed, and finishing an Active plan
//! whose last open task closed, live here too.

use std::collections::BTreeSet;

use z_engine_protocol::TaskId;
use z_engine_protocol::work::{Actor, BlockReason, PlanStatus, Task, TaskStatus, WorkEvent};

use super::Emitted;
use crate::graph::{dependents, is_closed, satisfies};
use crate::state::PlanState;

/// Done -> Pending (releasing the owner), by the kernel.
pub(super) fn reopen(task: &Task) -> Emitted {
    let mut out = vec![(
        Actor::Kernel,
        WorkEvent::TaskTransitioned {
            task_id: task.id.clone(),
            from: TaskStatus::Done,
            to: TaskStatus::Pending,
        },
    )];
    if task.owner.is_some() {
        out.push((
            Actor::Kernel,
            WorkEvent::TaskReleased {
                task_id: task.id.clone(),
            },
        ));
    }
    out
}

/// Block the open dependents of tasks that failed or were superseded in
/// this submission, and unblock tasks whose blocking dependency is
/// satisfied or gone.
pub(super) fn follow_ups(before: &PlanState, after: &PlanState) -> Vec<WorkEvent> {
    let mut out = Vec::new();
    let mut touched = BTreeSet::new();
    let newly_broken = after.tasks.values().filter_map(|task| {
        let reason = broken(task)?;
        let was = before.task(&task.id).and_then(broken).is_some();
        (!was).then_some((task, reason))
    });
    for (broken_task, reason) in newly_broken {
        for dependent in dependents(after, &broken_task.id) {
            let Some(task) = after.task(&dependent) else {
                continue;
            };
            let blockable = matches!(
                task.status,
                TaskStatus::Pending | TaskStatus::Active | TaskStatus::Verifying
            );
            if blockable && touched.insert(dependent.clone()) {
                out.push(blocked(task, reason.clone()));
            }
        }
    }
    for task in after.tasks.values() {
        let TaskStatus::Blocked {
            reason:
                BlockReason::Dependency { task_id: blocker }
                | BlockReason::DependencySuperseded {
                    task_id: blocker, ..
                },
        } = &task.status
        else {
            continue;
        };
        if touched.contains(&task.id) || !released(after, task, blocker) {
            continue;
        }
        let other = task
            .depends_on
            .iter()
            .filter(|dep| *dep != blocker)
            .find_map(|dep| after.task(dep).and_then(broken));
        out.push(match other {
            Some(reason) => blocked(task, reason),
            None => WorkEvent::TaskTransitioned {
                task_id: task.id.clone(),
                from: task.status.clone(),
                to: TaskStatus::Pending,
            },
        });
    }
    out
}

/// The submission closed the last open task of an Active plan: it is Done.
/// Reopening a Done plan closes nothing, so it stays Active.
pub(super) fn finished(before: &PlanState, after: &PlanState) -> Option<WorkEvent> {
    let open = |state: &PlanState| state.tasks.values().any(|task| !is_closed(&task.status));
    let done = after.header.status == PlanStatus::Active
        && !after.tasks.is_empty()
        && open(before)
        && !open(after);
    done.then_some(WorkEvent::PlanStatusChanged {
        from: PlanStatus::Active,
        to: PlanStatus::Done,
    })
}

/// The blocking dependency was removed, or it succeeded.
fn released(state: &PlanState, task: &Task, blocker: &TaskId) -> bool {
    !task.depends_on.contains(blocker)
        || state
            .task(blocker)
            .is_none_or(|blocker| satisfies(&blocker.status))
}

fn blocked(task: &Task, reason: BlockReason) -> WorkEvent {
    WorkEvent::TaskTransitioned {
        task_id: task.id.clone(),
        from: task.status.clone(),
        to: TaskStatus::Blocked { reason },
    }
}

/// How a failed or superseded dependency blocks its dependents.
fn broken(task: &Task) -> Option<BlockReason> {
    match &task.status {
        TaskStatus::Failed { .. } => Some(BlockReason::Dependency {
            task_id: task.id.clone(),
        }),
        TaskStatus::Superseded { by } => Some(BlockReason::DependencySuperseded {
            task_id: task.id.clone(),
            by: by.clone(),
        }),
        _ => None,
    }
}
