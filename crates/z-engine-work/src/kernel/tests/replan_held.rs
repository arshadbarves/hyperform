use z_engine_protocol::work::{
    PacketBlocker, ParallelBlock, ParallelPolicy, PlanStatus, TaskStatus, WorkEdit,
};

use super::run::send;
use crate::testkit::*;
use crate::{Proposal, Submission, decide, packet, parallel};

const HELD: &str = "held until the plan change is accepted";

#[test]
fn findings_hold_only_the_tasks_they_affect() {
    let mut state = reconciled(vec![deleted("a")]);
    assert_eq!(state.header.status, PlanStatus::Replanning);
    let facts = parallel_facts(3);
    let rejection = refuse_with(&state, main_agent(), vec![send("a", "w1", None)], &facts);
    assert!(rejection.to_string().contains(HELD), "{rejection}");
    let rejection = refuse(&state, main_agent(), vec![set("a", TaskStatus::Active)]);
    assert!(rejection.to_string().contains(HELD), "{rejection}");
    submit_with(
        &mut state,
        main_agent(),
        vec![send("c", "w1", None)],
        &facts,
    );
    assert_eq!(status_of(&state, "c"), TaskStatus::Active);
    assert_eq!(state.snapshot().replan_held, [tid("a"), tid("b")]);
}

#[test]
fn held_tasks_are_excluded_from_the_parallel_set_and_their_packets_say_so() {
    let mut state = reconciled(vec![deleted("a")]);
    let approve = Proposal::SetParallel {
        parallel: ParallelPolicy::Approved,
    };
    submit(&mut state, user(), vec![approve]);
    let set = parallel::eligible(&state, &parallel_facts(3));
    assert_eq!(set.eligible, [tid("c")]);
    let blocks: Vec<_> = set
        .excluded
        .iter()
        .map(|excluded| (excluded.task_id.clone(), excluded.block.clone()))
        .collect();
    assert_eq!(blocks, [(tid("a"), ParallelBlock::Replanning)]);
    let blocked = |id: &str| {
        packet::select(&state, &tid(id), 10_000, &facts(), &packet::estimate_chars)
            .unwrap()
            .blockers
            .contains(&PacketBlocker::Plan {
                status: PlanStatus::Replanning,
            })
    };
    assert!(blocked("b"));
    assert!(!blocked("c"));
}

#[test]
fn accepting_the_plan_change_releases_the_held_tasks() {
    let mut state = reconciled(vec![deleted("a")]);
    let approve = Submission::edit(WorkEdit::Approve);
    state.apply_all(&decide(&state, &approve, &facts()).unwrap());
    assert_eq!(state.header.status, PlanStatus::Active);
    assert!(state.snapshot().replan_held.is_empty());
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
}

#[test]
fn replanning_the_user_asked_for_still_holds_every_task() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let replan = Submission::edit(WorkEdit::RequestReplan);
    state.apply_all(&decide(&state, &replan, &facts()).unwrap());
    assert_eq!(state.header.status, PlanStatus::Replanning);
    let rejection = refuse(&state, main_agent(), vec![set("a", TaskStatus::Active)]);
    assert!(rejection.to_string().contains("the plan is"), "{rejection}");
    assert!(state.snapshot().replan_held.is_empty());
}
