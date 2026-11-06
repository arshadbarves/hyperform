use z_engine_protocol::work::{
    ParallelBlock, ParallelExcluded, ParallelPolicy, PlanStatus, TaskPatch, TaskStatus,
};

use super::*;
use crate::Proposal;
use crate::testkit::*;

fn approve(state: &mut PlanState) {
    submit(
        state,
        user(),
        vec![Proposal::SetParallel {
            parallel: ParallelPolicy::Approved,
        }],
    );
}

fn start(state: &mut PlanState, id: &str) {
    submit(state, main_agent(), vec![set(id, TaskStatus::Active)]);
}

fn excluded(id: &str, block: ParallelBlock) -> ParallelExcluded {
    ParallelExcluded {
        task_id: tid(id),
        block,
    }
}

fn three_apart(state: &mut PlanState) {
    scoped(state, "a", &[], &["src/a/**"]);
    scoped(state, "b", &[], &["src/b/**"]);
    scoped(state, "c", &[], &["src/c/**"]);
}

#[test]
fn nothing_runs_in_parallel_until_the_user_approves() {
    let mut state = plan(PlanStatus::Active);
    three_apart(&mut state);
    let set = eligible(&state, &parallel_facts(3));
    assert!(set.eligible.is_empty());
    let off = ParallelBlock::NotApproved {
        parallel: ParallelPolicy::Off,
    };
    assert_eq!(set.excluded[0], excluded("a", off));
    assert_eq!(set.excluded.len(), 3);
    assert_eq!(
        preview(&state, &parallel_facts(3)).eligible,
        [tid("a"), tid("b"), tid("c")]
    );
}

#[test]
fn plan_mode_blocks_parallel_work() {
    let mut state = plan(PlanStatus::Active);
    three_apart(&mut state);
    approve(&mut state);
    let facts = Facts {
        planning: true,
        ..parallel_facts(3)
    };
    let set = eligible(&state, &facts);
    assert!(set.eligible.is_empty());
    assert_eq!(set.excluded[0].block, ParallelBlock::Planning);
}

#[test]
fn a_draft_plan_previews_what_could_run_together() {
    let mut state = plan(PlanStatus::Draft);
    three_apart(&mut state);
    assert!(eligible(&state, &parallel_facts(3)).eligible.is_empty());
    assert_eq!(
        preview(&state, &parallel_facts(2)).eligible,
        [tid("a"), tid("b")]
    );
}

#[test]
fn independent_tasks_fill_the_free_slots() {
    let mut state = plan(PlanStatus::Active);
    three_apart(&mut state);
    approve(&mut state);
    let set = eligible(&state, &parallel_facts(2));
    assert_eq!(set.eligible, [tid("a"), tid("b")]);
    assert_eq!(
        set.excluded,
        [excluded("c", ParallelBlock::NoSlot { max: 2 })]
    );
    assert_eq!(set.max, 2);
    assert!(set.running.is_empty());
}

#[test]
fn running_tasks_take_slots_and_are_listed() {
    let mut state = plan(PlanStatus::Active);
    three_apart(&mut state);
    approve(&mut state);
    start(&mut state, "a");
    let set = eligible(&state, &parallel_facts(2));
    assert_eq!(set.running, [tid("a")]);
    assert_eq!(set.eligible, [tid("b")]);
    assert_eq!(
        set.excluded,
        [excluded("c", ParallelBlock::NoSlot { max: 2 })]
    );
}

#[test]
fn a_zero_limit_still_allows_one_task() {
    let mut state = plan(PlanStatus::Active);
    three_apart(&mut state);
    approve(&mut state);
    let set = eligible(&state, &parallel_facts(0));
    assert_eq!(set.eligible, [tid("a")]);
    assert_eq!(set.max, 1);
}

#[test]
fn overlapping_scopes_are_excluded_with_the_overlap() {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "a", &[], &["src/api/**"]);
    scoped(&mut state, "b", &[], &["src/api/client.rs"]);
    scoped(&mut state, "c", &[], &["docs/**"]);
    approve(&mut state);
    let set = eligible(&state, &parallel_facts(3));
    assert_eq!(set.eligible, [tid("a"), tid("c")]);
    assert_eq!(
        set.excluded,
        [excluded(
            "b",
            ParallelBlock::Overlap {
                task_id: tid("a"),
                files: vec!["src/api/client.rs".into()],
                symbols: vec![],
            }
        )]
    );
}

#[test]
fn shared_symbols_overlap_even_in_separate_files() {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "a", &[], &["src/a.rs"]);
    scoped(&mut state, "b", &[], &["src/b.rs"]);
    for (id, symbol) in [("a", "Retry"), ("b", "Retry")] {
        let mut scope = state.task(&tid(id)).unwrap().scope.clone();
        scope.symbols = vec![symbol.into()];
        let patch = TaskPatch {
            scope: Some(scope),
            ..TaskPatch::default()
        };
        submit(
            &mut state,
            user(),
            vec![Proposal::EditTask {
                task_id: tid(id),
                patch: Box::new(patch),
            }],
        );
    }
    approve(&mut state);
    let set = eligible(&state, &parallel_facts(3));
    assert_eq!(
        set.excluded[0].block,
        ParallelBlock::Overlap {
            task_id: tid("a"),
            files: vec![],
            symbols: vec!["Retry".into()],
        }
    );
}

#[test]
fn tasks_without_files_or_on_serial_paths_or_alone_run_by_themselves() {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "free", &[], &["src/free/**"]);
    scoped(&mut state, "blind", &[], &[]);
    scoped(&mut state, "lock", &[], &["crates/app/Cargo.toml"]);
    scoped(&mut state, "dir", &[], &["crates/app/**"]);
    submit(
        &mut state,
        user(),
        vec![Proposal::EditTask {
            task_id: tid("dir"),
            patch: Box::new(TaskPatch {
                run_alone: Some(true),
                ..TaskPatch::default()
            }),
        }],
    );
    approve(&mut state);
    let set = eligible(&state, &parallel_facts(4));
    assert_eq!(set.eligible, [tid("free")]);
    assert_eq!(
        set.excluded,
        [
            excluded(
                "blind",
                ParallelBlock::UnknownScope {
                    task_id: tid("blind")
                }
            ),
            excluded(
                "lock",
                ParallelBlock::SerialPaths {
                    task_id: tid("lock"),
                    patterns: vec!["**/Cargo.toml".into()],
                }
            ),
            excluded(
                "dir",
                ParallelBlock::RunAlone {
                    task_id: tid("dir")
                }
            ),
        ]
    );
}

#[test]
fn a_lone_task_may_start_when_nothing_else_runs() {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "blind", &[], &[]);
    scoped(&mut state, "free", &[], &["src/free/**"]);
    approve(&mut state);
    let set = eligible(&state, &parallel_facts(3));
    assert_eq!(set.eligible, [tid("blind")]);
    assert_eq!(
        set.excluded,
        [excluded(
            "free",
            ParallelBlock::UnknownScope {
                task_id: tid("blind")
            }
        )]
    );
}

#[test]
fn a_running_task_on_serial_paths_keeps_the_others_waiting() {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "lock", &[], &["Cargo.lock"]);
    scoped(&mut state, "free", &[], &["src/free/**"]);
    approve(&mut state);
    start(&mut state, "lock");
    let set = eligible(&state, &parallel_facts(3));
    assert!(set.eligible.is_empty());
    assert_eq!(
        set.excluded[0].block,
        ParallelBlock::SerialPaths {
            task_id: tid("lock"),
            patterns: vec!["**/Cargo.lock".into()],
        }
    );
}

#[test]
fn tasks_linked_through_a_cancelled_dependency_stay_ordered() {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "a", &[], &["src/a/**"]);
    scoped(&mut state, "x", &["a"], &["src/x/**"]);
    scoped(&mut state, "c", &["x"], &["src/c/**"]);
    approve(&mut state);
    start(&mut state, "a");
    submit(&mut state, user(), vec![set("x", TaskStatus::Cancelled)]);
    let set = eligible(&state, &parallel_facts(3));
    assert_eq!(
        set.excluded,
        [excluded("c", ParallelBlock::Ordered { task_id: tid("a") })]
    );
}

#[test]
fn beside_running_checks_only_when_another_task_runs() {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "a", &[], &["src/a/**"]);
    scoped(&mut state, "b", &[], &["src/a/b.rs"]);
    scoped(&mut state, "c", &[], &["src/c/**"]);
    let b = state.task(&tid("b")).unwrap().clone();
    assert_eq!(beside_running(&state, &b, &parallel_facts(3)), None);
    start(&mut state, "a");
    let off = ParallelBlock::NotApproved {
        parallel: ParallelPolicy::Off,
    };
    assert_eq!(beside_running(&state, &b, &parallel_facts(3)), Some(off));
    approve(&mut state);
    assert!(matches!(
        beside_running(&state, &b, &parallel_facts(3)),
        Some(ParallelBlock::Overlap { .. })
    ));
    let c = state.task(&tid("c")).unwrap().clone();
    assert_eq!(beside_running(&state, &c, &parallel_facts(3)), None);
    assert_eq!(
        beside_running(&state, &c, &parallel_facts(1)),
        Some(ParallelBlock::NoSlot { max: 1 })
    );
}

#[test]
fn every_block_reads_as_a_sentence() {
    let text = describe(&ParallelBlock::Overlap {
        task_id: tid("a"),
        files: vec!["src/a.rs".into()],
        symbols: vec!["Retry".into()],
    });
    assert_eq!(text, "its scope overlaps task a (src/a.rs, Retry)");
    assert_eq!(
        describe(&ParallelBlock::NoSlot { max: 3 }),
        "3 tasks already run (work.max_parallel)"
    );
    assert!(
        describe(&ParallelBlock::NotApproved {
            parallel: ParallelPolicy::Ask
        })
        .contains("waiting for approval")
    );
}
