//! A failure holds only what it affects. While reconciliation findings
//! keep the plan Replanning, the open tasks they name and every task that
//! depends on one of them are held until the user accepts the plan change;
//! the other tasks stay ready. Replanning the user asked for (planning
//! turned on, a replan request) still holds every task through the plan
//! status, as do Draft and Paused.

use std::collections::BTreeSet;

use z_engine_protocol::TaskId;
use z_engine_protocol::work::{PlanStatus, ReconcileFinding};

use crate::graph::{is_open, transitive_dependencies, transitive_dependents};
use crate::state::PlanState;

/// Findings moved the plan to Replanning: unaffected tasks may still run.
pub fn partial(state: &PlanState) -> bool {
    state.header.status == PlanStatus::Replanning && state.replan_by_findings
}

/// The open tasks held until the plan change is accepted; empty unless
/// findings keep the plan Replanning.
pub fn replan_held(state: &PlanState) -> BTreeSet<TaskId> {
    if !partial(state) {
        return BTreeSet::new();
    }
    let mut held = BTreeSet::new();
    for id in state.findings.iter().flat_map(ReconcileFinding::tasks) {
        held.insert(id.clone());
        held.extend(transitive_dependents(state, id));
    }
    held.retain(|id| state.task(id).is_some_and(|task| is_open(&task.status)));
    held
}

/// `id` is held by the findings: one names it or a task it depends on.
pub fn holds(state: &PlanState, id: &TaskId) -> bool {
    if !partial(state) {
        return false;
    }
    let mut upstream = transitive_dependencies(state, id);
    upstream.insert(id.clone());
    state
        .findings
        .iter()
        .flat_map(ReconcileFinding::tasks)
        .any(|named| upstream.contains(named))
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::work::PlanStatus;

    use super::*;
    use crate::proposal::Proposal;
    use crate::testkit::{deleted, reconciled, submit, tid, user};

    #[test]
    fn findings_hold_the_tasks_they_name_and_their_dependents_only() {
        let state = reconciled(vec![deleted("a")]);
        assert_eq!(state.header.status, PlanStatus::Replanning);
        assert!(state.replan_by_findings);
        let held: Vec<TaskId> = replan_held(&state).into_iter().collect();
        assert_eq!(held, vec![tid("a"), tid("b")]);
        assert!(holds(&state, &tid("b")) && !holds(&state, &tid("c")));
    }

    #[test]
    fn replanning_the_user_asked_for_holds_nothing_in_particular() {
        let mut state = reconciled(vec![deleted("a")]);
        submit(
            &mut state,
            user(),
            vec![Proposal::SetPlanStatus {
                status: PlanStatus::Paused,
                only_from: vec![],
            }],
        );
        submit(
            &mut state,
            user(),
            vec![Proposal::SetPlanStatus {
                status: PlanStatus::Replanning,
                only_from: vec![],
            }],
        );
        assert!(!state.replan_by_findings);
        assert!(replan_held(&state).is_empty());
        assert!(!holds(&state, &tid("a")));
    }
}
