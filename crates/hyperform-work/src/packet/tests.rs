use hyperform_protocol::verification::CheckKind;
use hyperform_protocol::work::{
    Actor, BlockReason, Discovery, EntryBody, EntryStatus, PacketBlocker, PacketReason,
    PlanItemKind, PlanStatus, TaskScope, TaskStatus,
};
use hyperform_protocol::{EntryId, TaskId};

use super::*;
use crate::testkit::*;
use crate::{EntryDraft, Proposal, TaskDraft};

fn note(id: &str, tasks: &[&str], files: &[&str]) -> Proposal {
    Proposal::Record(Box::new(EntryDraft {
        id: Some(EntryId::from(id)),
        tasks: tasks.iter().map(|t| tid(t)).collect(),
        files: files.iter().map(|f| f.to_string()).collect(),
        ..EntryDraft::new(EntryBody::Discovery(Discovery {
            text: format!("note {id}"),
            source: None,
        }))
    }))
}

/// `p` > `t` (scope src/t.rs, depends on `d`), plus `other`; one entry per
/// relevance tier and some that must be left out.
fn rich() -> PlanState {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![
            Proposal::AddItem {
                id: Some(EntryId::from("ent_inv")),
                kind: PlanItemKind::Invariant,
                text: "never lose data".into(),
            },
            Proposal::AddItem {
                id: Some(EntryId::from("ent_req")),
                kind: PlanItemKind::Requirement,
                text: "works offline".into(),
            },
            create_task(task("p")),
            create_task(task("d")),
            create_task(task("other")),
        ],
    );
    let child = TaskDraft {
        parent: Some(tid("p")),
        depends_on: vec![tid("d")],
        requirements: vec![EntryId::from("ent_req")],
        scope: TaskScope {
            files: vec!["src/t.rs".into()],
            symbols: vec![],
        },
        acceptance: vec![check_criterion("tests pass", CheckKind::Test)],
        ..task("t")
    };
    submit(&mut state, user(), vec![create_task(child)]);
    submit(&mut state, user(), vec![note("ent_1recent", &[], &[])]);
    submit(
        &mut state,
        user(),
        vec![note("ent_2scope", &[], &["src/t.rs"])],
    );
    submit(&mut state, user(), vec![note("ent_3dep", &["d"], &[])]);
    submit(&mut state, user(), vec![note("ent_4anc", &["p"], &[])]);
    submit(&mut state, user(), vec![note("ent_5task", &["t"], &[])]);
    submit(
        &mut state,
        user(),
        vec![note("ent_6other", &["other"], &[])],
    );
    submit(&mut state, user(), vec![note("ent_7gone", &["t"], &[])]);
    submit(
        &mut state,
        user(),
        vec![Proposal::SetEntryStatus {
            entry_id: EntryId::from("ent_7gone"),
            status: EntryStatus::Dismissed,
        }],
    );
    state
}

fn reasons(packet: &TaskPacket) -> Vec<(&str, PacketReason)> {
    packet
        .entries
        .iter()
        .map(|e| (e.entry.id.as_str(), e.reason))
        .collect()
}

#[test]
fn entries_are_ranked_by_relevance_and_unrelated_ones_left_out() {
    let state = rich();
    let packet = select(&state, &tid("t"), 100_000, &facts(), &estimate_chars).unwrap();
    assert_eq!(
        reasons(&packet),
        [
            ("ent_5task", PacketReason::Task),
            ("ent_4anc", PacketReason::Ancestor),
            ("ent_3dep", PacketReason::Dependency),
            ("ent_2scope", PacketReason::Scope),
            ("ent_1recent", PacketReason::Recent),
        ]
    );
    assert_eq!(packet.dropped, 0);
    assert_eq!(packet.goal, "Ship it");
    assert_eq!(packet.invariants[0].text, "never lose data");
    assert_eq!(packet.requirements[0].text, "works offline");
    assert_eq!(packet.parent.as_ref().unwrap().id, tid("p"));
    assert_eq!(packet.dependencies[0].id, tid("d"));
    assert_eq!(packet.scope, ["src/t.rs"]);
    assert_eq!(packet.next_ready.as_ref().unwrap().id, tid("p"));
}

#[test]
fn newer_entries_come_first_within_a_tier() {
    let mut state = rich();
    submit(&mut state, user(), vec![note("ent_0newer", &["t"], &[])]);
    let packet = select(&state, &tid("t"), 100_000, &facts(), &estimate_chars).unwrap();
    assert_eq!(packet.entries[0].entry.id.as_str(), "ent_0newer");
    assert_eq!(packet.entries[1].entry.id.as_str(), "ent_5task");
}

#[test]
fn a_tight_budget_drops_optional_parts_but_never_the_core() {
    let state = rich();
    let full = select(&state, &tid("t"), 100_000, &facts(), &estimate_chars).unwrap();
    let tight = select(&state, &tid("t"), 0, &facts(), &estimate_chars).unwrap();
    assert!(tight.entries.is_empty() && tight.dependencies.is_empty());
    assert_eq!(tight.dropped, 6, "one dependency and five entries");
    assert_eq!(
        (tight.task.id.clone(), tight.goal.clone()),
        (tid("t"), "Ship it".into())
    );
    assert_eq!(tight.invariants.len(), 1);
    assert!(tight.tokens > tight.budget, "the core is always sent");
    assert!(tight.tokens < full.tokens);
    let core = tight.tokens;
    let some = select(
        &state,
        &tid("t"),
        core + full.entries[0].tokens + 4,
        &facts(),
        &estimate_chars,
    )
    .unwrap();
    assert!(!some.dependencies.is_empty() || !some.entries.is_empty());
    assert!(some.tokens <= some.budget);
}

#[test]
fn a_custom_estimator_is_used() {
    let state = rich();
    let flat = |_: &str| 10;
    let packet = select(&state, &tid("t"), 35, &facts(), &flat).unwrap();
    assert_eq!(packet.tokens, 30, "core, the dependency, one entry");
    assert_eq!(packet.entries.len(), 1);
    assert_eq!(packet.dropped, 4);
    assert_eq!(estimate_chars("abcde"), 2);
}

#[test]
fn blockers_say_why_the_task_cannot_move() {
    let mut state = rich();
    submit(
        &mut state,
        user(),
        vec![
            Proposal::HoldTask {
                task_id: tid("t"),
                held: true,
            },
            Proposal::from(hyperform_protocol::work::WorkEdit::Pause),
        ],
    );
    let packet = select(&state, &tid("t"), 100_000, &facts(), &estimate_chars).unwrap();
    assert!(matches!(&packet.blockers[0], PacketBlocker::Waiting { task } if task.id == tid("d")));
    assert_eq!(packet.blockers[1], PacketBlocker::Held);
    assert_eq!(
        packet.blockers[2],
        PacketBlocker::Plan {
            status: PlanStatus::Paused
        }
    );
    let mut blocked = rich();
    blocked.tasks.get_mut(&tid("t")).unwrap().status = TaskStatus::Blocked {
        reason: BlockReason::Unreported,
    };
    let packet = select(&blocked, &tid("t"), 100_000, &facts(), &estimate_chars).unwrap();
    assert!(matches!(packet.blockers[0], PacketBlocker::Blocked { .. }));
}

#[test]
fn the_latest_evidence_per_criterion_carries_its_freshness() {
    let mut state = rich();
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("t", "c1", check("r1", true, "fp0"))],
    );
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("t", "c1", check("r2", true, FP))],
    );
    let packet = select(&state, &tid("t"), 100_000, &facts(), &estimate_chars).unwrap();
    assert_eq!(packet.evidence.len(), 1);
    assert!(!packet.evidence[0].stale);
    let stale = Facts {
        stale_records: ["r2".to_string()].into(),
        ..facts()
    };
    let packet = select(&state, &tid("t"), 100_000, &stale, &estimate_chars).unwrap();
    assert!(packet.evidence[0].stale);
}

#[test]
fn unknown_tasks_are_an_error() {
    let state = rich();
    assert_eq!(
        select(&state, &TaskId::from("nope"), 10, &facts(), &estimate_chars),
        Err(PacketError::UnknownTask(TaskId::from("nope")))
    );
}
