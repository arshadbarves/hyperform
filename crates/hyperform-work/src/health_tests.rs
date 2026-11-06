use hyperform_protocol::EntryId;
use hyperform_protocol::verification::CheckKind;
use hyperform_protocol::work::{
    AuditFinding, AuditKind, AuditSeverity, AuditSource, BlockReason, PlanItemKind, PlanStatus,
    ReconcileFinding,
};

use super::*;
use crate::testkit::*;
use crate::{Proposal, TaskDraft};

fn with_criterion(id: &str) -> TaskDraft {
    TaskDraft {
        acceptance: vec![check_criterion("tests pass", CheckKind::Test)],
        ..task(id)
    }
}

fn force(state: &mut PlanState, id: &str, status: TaskStatus) {
    state.tasks.get_mut(&tid(id)).unwrap().status = status;
}

fn criteria(state: &mut PlanState, id: &str, status: CriterionStatus) {
    for criterion in &mut state.tasks.get_mut(&tid(id)).unwrap().acceptance {
        criterion.status = status;
    }
}

fn finding(id: &str, status: AuditStatus) -> AuditFinding {
    AuditFinding {
        id: id.into(),
        kind: AuditKind::TaskWithoutCriteria,
        severity: AuditSeverity::Medium,
        text: "x".into(),
        refs: vec![],
        suggestion: None,
        status,
        source: AuditSource::Deterministic,
        created_revision: 1,
        revision: 1,
    }
}

#[test]
fn an_empty_plan_has_zero_counts_and_fair_evidence() {
    assert_eq!(health(&plan(PlanStatus::Active)), PlanHealth::default());
    assert_eq!(PlanHealth::default().evidence, HealthLevel::Fair);
}

#[test]
fn counts_cover_requirements_tasks_and_trouble() {
    let mut state = plan(PlanStatus::Active);
    for id in ["r1", "r2"] {
        submit(
            &mut state,
            user(),
            vec![Proposal::AddItem {
                id: Some(EntryId::from(id)),
                kind: PlanItemKind::Requirement,
                text: id.into(),
            }],
        );
    }
    let serving = TaskDraft {
        requirements: vec![EntryId::from("r1")],
        ..with_criterion("a")
    };
    submit(&mut state, user(), vec![create_task(serving)]);
    for id in ["b", "c", "d", "e"] {
        add(&mut state, id, &[]);
    }
    let blocked = TaskStatus::Blocked {
        reason: BlockReason::Unreported,
    };
    force(&mut state, "c", blocked);
    force(&mut state, "d", TaskStatus::Failed { reason: "x".into() });
    force(&mut state, "e", TaskStatus::Cancelled);
    let health = health(&state);
    assert_eq!((health.requirements, health.requirements_covered), (2, 1));
    assert_eq!((health.tasks, health.tasks_done), (3, 0));
    assert_eq!(health.tasks_without_criteria, 2);
    assert_eq!((health.blocked, health.failed), (1, 1));
}

#[test]
fn evidence_is_good_only_when_every_done_criterion_is_met() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![
            create_task(with_criterion("a")),
            create_task(with_criterion("b")),
        ],
    );
    force(&mut state, "a", TaskStatus::Done);
    criteria(&mut state, "a", CriterionStatus::Met);
    assert_eq!(health(&state).evidence, HealthLevel::Good);
    assert_eq!(health(&state).tasks_done, 1);
    force(&mut state, "b", TaskStatus::Done);
    criteria(&mut state, "b", CriterionStatus::Waived);
    assert_eq!(health(&state).evidence, HealthLevel::Fair);
    criteria(&mut state, "b", CriterionStatus::Failed);
    let stale = health(&state);
    assert_eq!(
        (stale.evidence, stale.evidence_stale),
        (HealthLevel::Poor, 1)
    );
}

#[test]
fn a_done_task_without_criteria_makes_evidence_poor() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    force(&mut state, "a", TaskStatus::Done);
    assert_eq!(health(&state).evidence, HealthLevel::Poor);
}

#[test]
fn open_findings_and_unmitigated_risks_are_counted() {
    let mut state = plan(PlanStatus::Active);
    let risky = TaskDraft {
        notes: hyperform_protocol::work::TaskNotes {
            risks: vec!["data loss".into()],
            ..Default::default()
        },
        ..with_criterion("a")
    };
    submit(&mut state, user(), vec![create_task(risky)]);
    state.audit = vec![
        finding("f1", AuditStatus::Open),
        finding("f2", AuditStatus::Dismissed),
        finding("f3", AuditStatus::Added),
    ];
    state.findings = vec![ReconcileFinding::Duplicate {
        tasks: vec![tid("a")],
        title: "A".into(),
    }];
    let health = health(&state);
    assert_eq!(health.open_findings, 1);
    assert_eq!(health.reconcile_findings, 1);
    assert_eq!(health.unmitigated_risks, 1);
}
