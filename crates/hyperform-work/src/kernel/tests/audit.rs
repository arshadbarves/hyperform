use hyperform_protocol::EntryId;
use hyperform_protocol::verification::CheckKind;
use hyperform_protocol::work::{
    Actor, AuditFinding, AuditKind, AuditRef, AuditSeverity, AuditSource, AuditStatus,
    PlanItemKind, PlanStatus, WorkEdit, WorkEvent,
};

use crate::testkit::*;
use crate::{AuditInput, AuditScope, Changes, Facts, PlanState, Proposal, Reason, audit};

fn deterministic(state: &PlanState) -> Proposal {
    Proposal::Audit {
        source: AuditSource::Deterministic,
        findings: audit(
            state,
            &AuditInput {
                check_kinds: None,
                risky_paths: &[],
            },
        ),
    }
}

fn review(text: &str, refs: Vec<AuditRef>) -> AuditFinding {
    AuditFinding {
        id: String::new(),
        kind: AuditKind::ModelReview,
        severity: AuditSeverity::Medium,
        text: text.into(),
        refs,
        suggestion: None,
        status: AuditStatus::Open,
        source: AuditSource::AuditAgent,
        created_revision: 0,
        revision: 0,
    }
}

fn reviewed(findings: Vec<AuditFinding>) -> Proposal {
    Proposal::Audit {
        source: AuditSource::AuditAgent,
        findings,
    }
}

fn status(state: &PlanState, id: &str) -> AuditStatus {
    state.finding(id).expect("finding").status
}

const BARE: &str = "taskWithoutCriteria:a";

/// Task `a` has no criteria and its finding is recorded.
fn audited() -> PlanState {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let proposal = deterministic(&state);
    submit(&mut state, Actor::Kernel, vec![proposal]);
    state
}

fn criteria_for_a() -> Proposal {
    Proposal::SetCriteria {
        task_id: tid("a"),
        criteria: vec![check_criterion("tests pass", CheckKind::Test)],
    }
}

#[test]
fn only_the_engine_records_findings() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let rejection = refuse(&state, main_agent(), vec![deterministic(&state)]);
    assert!(matches!(rejection.reasons[0], Reason::EngineOnly(_)));
    let rejection = refuse(&state, user(), vec![deterministic(&state)]);
    assert!(matches!(rejection.reasons[0], Reason::EngineOnly(_)));
}

#[test]
fn findings_keep_the_users_status_across_audits() {
    let mut state = audited();
    let finding = state.finding(BARE).unwrap();
    assert_eq!(finding.status, AuditStatus::Open);
    assert_eq!(finding.created_revision, state.revision());
    let dismiss = Proposal::from(WorkEdit::DismissFinding {
        finding_id: BARE.into(),
    });
    submit(&mut state, user(), vec![dismiss]);
    let again = deterministic(&state);
    assert!(submit(&mut state, Actor::Kernel, vec![again]).is_empty());
    assert_eq!(status(&state, BARE), AuditStatus::Dismissed);
}

#[test]
fn a_deterministic_audit_resolves_what_it_no_longer_finds() {
    let mut state = audited();
    submit(&mut state, user(), vec![criteria_for_a()]);
    let again = deterministic(&state);
    submit(&mut state, Actor::Kernel, vec![again]);
    assert_eq!(status(&state, BARE), AuditStatus::Resolved);
    let quiet = deterministic(&state);
    assert!(submit(&mut state, Actor::Kernel, vec![quiet]).is_empty());
    submit(
        &mut state,
        user(),
        vec![Proposal::SetCriteria {
            task_id: tid("a"),
            criteria: vec![],
        }],
    );
    let back = deterministic(&state);
    submit(&mut state, Actor::Kernel, vec![back]);
    assert_eq!(status(&state, BARE), AuditStatus::Open);
    assert_eq!(state.audit.len(), 1);
}

#[test]
fn a_dismissed_finding_stays_dismissed_when_it_goes_away() {
    let mut state = audited();
    let dismiss = Proposal::SetFindingStatus {
        finding_id: BARE.into(),
        status: AuditStatus::Dismissed,
    };
    submit(&mut state, user(), vec![dismiss]);
    submit(&mut state, user(), vec![criteria_for_a()]);
    let again = deterministic(&state);
    submit(&mut state, Actor::Kernel, vec![again]);
    assert_eq!(status(&state, BARE), AuditStatus::Dismissed);
}

#[test]
fn model_reviews_get_ids_and_leave_deterministic_findings_alone() {
    let mut state = audited();
    let on_a = || vec![AuditRef::Task { task_id: tid("a") }];
    submit(
        &mut state,
        Actor::Kernel,
        vec![reviewed(vec![
            review("No rollback for the schema change", on_a()),
            review("Retry storms are untested", on_a()),
        ])],
    );
    let ids: Vec<&str> = state.audit.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, [BARE, "review-1", "review-2"]);
    assert_eq!(status(&state, BARE), AuditStatus::Open);
    submit(
        &mut state,
        Actor::Kernel,
        vec![reviewed(vec![review("One more", on_a())])],
    );
    assert!(state.finding("review-3").is_some());
}

#[test]
fn findings_must_be_well_formed_and_name_known_things() {
    let state = audited();
    let refused = |source: AuditSource, finding: AuditFinding| {
        let proposal = Proposal::Audit {
            source,
            findings: vec![finding],
        };
        refuse(&state, Actor::Kernel, vec![proposal]).reasons
    };
    let blank = review("  ", vec![]);
    assert!(matches!(
        refused(AuditSource::AuditAgent, blank)[0],
        Reason::Invalid(_)
    ));
    let posing = review("x", vec![]);
    assert!(matches!(
        refused(AuditSource::Deterministic, posing)[0],
        Reason::Invalid(_)
    ));
    let mislabelled = AuditFinding {
        kind: AuditKind::Overlap,
        ..review("x", vec![])
    };
    assert!(matches!(
        refused(AuditSource::AuditAgent, mislabelled)[0],
        Reason::Invalid(_)
    ));
    let refs = vec![
        AuditRef::Task { task_id: tid("zz") },
        AuditRef::Criterion {
            task_id: tid("a"),
            criterion_id: "c9".into(),
        },
        AuditRef::Item {
            item_id: EntryId::from("r9"),
        },
        AuditRef::File { path: "any".into() },
    ];
    assert_eq!(
        refused(AuditSource::AuditAgent, review("x", refs)),
        [
            Reason::UnknownTask(tid("zz")),
            Reason::UnknownCriterion {
                task_id: tid("a"),
                criterion_id: "c9".into(),
            },
            Reason::UnknownItem(EntryId::from("r9")),
        ]
    );
}

#[test]
fn only_the_user_adds_or_dismisses_findings() {
    let mut state = audited();
    let add = |id: &str| {
        Proposal::from(WorkEdit::AddFinding {
            finding_id: id.into(),
        })
    };
    assert!(refuse(&state, main_agent(), vec![add(BARE)]).needs_user());
    assert_eq!(
        refuse(&state, user(), vec![add("nope")]).reasons,
        [Reason::UnknownFinding("nope".into())]
    );
    let resolve = Proposal::SetFindingStatus {
        finding_id: BARE.into(),
        status: AuditStatus::Resolved,
    };
    assert!(matches!(
        refuse(&state, user(), vec![resolve]).reasons[0],
        Reason::Invalid(_)
    ));
    let records = submit(&mut state, user(), vec![add(BARE)]);
    assert_eq!(status(&state, BARE), AuditStatus::Added);
    assert!(submit(&mut state, user(), vec![add(BARE)]).is_empty());
    let update = state.update(&Changes::default());
    assert_eq!(update.audit[0].status, AuditStatus::Added);
    assert_eq!(update.health.open_findings, 0);
    assert_eq!(records.len(), 1);
}

#[test]
fn with_an_audit_scope_every_revision_ends_with_the_audit_when_it_changes() {
    let facts = Facts {
        audit: Some(AuditScope::default()),
        ..crate::testkit::facts()
    };
    let mut state = plan(PlanStatus::Active);
    let draft = create_task(task("a"));
    let records = submit_with(&mut state, user(), vec![draft], &facts);
    let last = records.last().unwrap();
    assert!(matches!(last.event, WorkEvent::Audited { .. }));
    assert_eq!(
        (last.actor.clone(), last.revision),
        (Actor::Kernel, records[0].revision)
    );
    assert_eq!(status(&state, BARE), AuditStatus::Open);
    let records = submit_with(&mut state, user(), vec![approve_parallel()], &facts);
    assert!(
        records
            .iter()
            .all(|record| !matches!(record.event, WorkEvent::Audited { .. })),
        "unchanged findings are not recorded again"
    );
    let records = submit_with(&mut state, user(), vec![criteria_for_a()], &facts);
    assert!(matches!(
        records.last().unwrap().event,
        WorkEvent::Audited { .. }
    ));
    assert_eq!(status(&state, BARE), AuditStatus::Resolved);
    let records = submit_with(&mut state, user(), vec![add_requirement()], &facts);
    let audited = records
        .iter()
        .filter(|record| matches!(record.event, WorkEvent::Audited { .. }));
    assert_eq!(audited.count(), 1, "one audit per revision");
}

fn approve_parallel() -> Proposal {
    Proposal::from(WorkEdit::ApproveParallel)
}

fn add_requirement() -> Proposal {
    Proposal::from(WorkEdit::AddItem {
        kind: PlanItemKind::Requirement,
        text: "Old files stay readable".into(),
    })
}
