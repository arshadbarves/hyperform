use z_engine_protocol::work::{PlanStatus, TaskPriority, TaskStatus};

use super::*;
use crate::testkit::*;
use crate::{Proposal, TaskDraft};

fn ids(items: &[&str]) -> Vec<TaskId> {
    items.iter().map(|id| tid(id)).collect()
}

#[test]
fn readiness_is_derived_from_dependencies_holds_and_the_plan() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    assert_eq!(readiness(&state, &tid("a")), Some(Readiness::Ready));
    assert_eq!(
        readiness(&state, &tid("b")),
        Some(Readiness::Waiting { on: ids(&["a"]) })
    );
    submit(&mut state, user(), vec![set("a", TaskStatus::Active)]);
    assert_eq!(readiness(&state, &tid("a")), Some(Readiness::NotPending));
    submit(&mut state, user(), vec![set("a", TaskStatus::Done)]);
    assert_eq!(readiness(&state, &tid("b")), Some(Readiness::Ready));
    submit(
        &mut state,
        user(),
        vec![Proposal::HoldTask {
            task_id: tid("b"),
            held: true,
        }],
    );
    assert_eq!(readiness(&state, &tid("b")), Some(Readiness::Held));
    state.header.status = PlanStatus::Paused;
    add(&mut state, "c", &[]);
    assert_eq!(
        readiness(&state, &tid("c")),
        Some(Readiness::PlanNotActive(PlanStatus::Paused))
    );
    assert_eq!(readiness(&state, &tid("ghost")), None);
}

#[test]
fn cancelled_dependencies_no_longer_hold_dependents_back() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    submit(&mut state, user(), vec![set("a", TaskStatus::Cancelled)]);
    assert_eq!(readiness(&state, &tid("b")), Some(Readiness::Ready));
    assert!(satisfies(&TaskStatus::Done) && satisfies(&TaskStatus::Cancelled));
    assert!(!satisfies(&TaskStatus::Superseded { by: tid("x") }));
}

#[test]
fn ready_tasks_put_higher_priority_first_then_tree_order() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    submit(
        &mut state,
        user(),
        vec![create_task(TaskDraft {
            priority: TaskPriority::High,
            ..task("c")
        })],
    );
    add(&mut state, "d", &["a"]);
    assert_eq!(ready_tasks(&state), ids(&["c", "a", "b"]));
    assert_eq!(next_ready(&state, Some(&tid("c"))).unwrap().id, tid("a"));
}

#[test]
fn tree_order_is_depth_first_by_sibling_order() {
    let mut state = plan(PlanStatus::Draft);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &[]);
    for (id, parent) in [("a1", "a"), ("a2", "a"), ("b1", "b")] {
        submit(
            &mut state,
            user(),
            vec![create_task(TaskDraft {
                parent: Some(tid(parent)),
                ..task(id)
            })],
        );
    }
    assert_eq!(tree_order(&state), ids(&["a", "a1", "a2", "b", "b1"]));
    assert_eq!(
        children(&state, &tid("a"))
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>(),
        ids(&["a1", "a2"])
    );
    assert_eq!(ancestors(&state, &tid("a2")), ids(&["a"]));
    assert_eq!(
        descendants(&state, &tid("a"))
            .into_iter()
            .collect::<Vec<_>>(),
        ids(&["a1", "a2"])
    );
}

#[test]
fn dependents_and_dependencies_are_walked_transitively() {
    let mut state = plan(PlanStatus::Draft);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    add(&mut state, "c", &["b"]);
    add(&mut state, "d", &[]);
    assert_eq!(dependents(&state, &tid("a")), ids(&["b"]));
    assert_eq!(
        transitive_dependents(&state, &tid("a"))
            .into_iter()
            .collect::<Vec<_>>(),
        ids(&["b", "c"])
    );
    assert_eq!(
        transitive_dependencies(&state, &tid("c"))
            .into_iter()
            .collect::<Vec<_>>(),
        ids(&["a", "b"])
    );
    assert!(ordered(&state, &tid("a"), &tid("c")));
    assert!(ordered(&state, &tid("c"), &tid("a")));
    assert!(!ordered(&state, &tid("a"), &tid("d")));
}

#[test]
fn cycles_are_found_before_they_are_added() {
    let mut state = plan(PlanStatus::Draft);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    add(&mut state, "c", &["b"]);
    assert_eq!(
        cycle_if_added(&state, &tid("a"), &tid("a")),
        Some(ids(&["a", "a"]))
    );
    assert_eq!(
        cycle_if_added(&state, &tid("a"), &tid("c")),
        Some(ids(&["a", "c", "b", "a"]))
    );
    assert_eq!(cycle_if_added(&state, &tid("c"), &tid("a")), None);
    assert_eq!(find_cycle(&state), None);
}

#[test]
fn a_corrupt_cycle_is_reported_and_does_not_hang_traversals() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    add(&mut state, "b", &["a"]);
    state
        .tasks
        .get_mut(&tid("a"))
        .unwrap()
        .depends_on
        .push(tid("b"));
    state.tasks.get_mut(&tid("a")).unwrap().parent = Some(tid("b"));
    state.tasks.get_mut(&tid("b")).unwrap().parent = Some(tid("a"));
    let cycle = find_cycle(&state).expect("cycle");
    assert_eq!(cycle.first(), cycle.last());
    assert_eq!(transitive_dependents(&state, &tid("a")).len(), 1);
    assert_eq!(ancestors(&state, &tid("a")), ids(&["b"]));
    assert_eq!(tree_order(&state).len(), 2, "orphaned loops still appear");
    assert!(ready_tasks(&state).is_empty());
}

#[test]
fn open_and_closed_partition_planned_statuses() {
    use TaskStatus::*;
    for status in [Pending, Active, Verifying] {
        assert!(is_open(&status) && !is_closed(&status));
    }
    for status in [Done, Cancelled, Superseded { by: tid("x") }] {
        assert!(is_closed(&status) && !is_open(&status));
    }
    assert!(!is_open(&Proposed) && !is_closed(&Proposed));
}
