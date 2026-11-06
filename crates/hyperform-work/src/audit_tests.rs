use hyperform_protocol::EntryId;
use hyperform_protocol::verification::CheckKind;
use hyperform_protocol::work::{
    AuditFinding, CriterionDraft, CriterionMethod, PlanItemKind, PlanStatus, TaskNotes, TaskScope,
    TaskStatus,
};

use super::*;
use crate::testkit::*;
use crate::{Proposal, TaskDraft};

fn ids(findings: &[AuditFinding]) -> Vec<String> {
    findings.iter().map(|finding| finding.id.clone()).collect()
}

fn risky() -> Vec<String> {
    RISKY_PATHS.iter().map(|path| path.to_string()).collect()
}

fn run(state: &PlanState) -> Vec<AuditFinding> {
    let risky = risky();
    audit(
        state,
        &AuditInput {
            check_kinds: None,
            risky_paths: &risky,
        },
    )
}

fn add_task(state: &mut PlanState, draft: TaskDraft) {
    submit(state, user(), vec![create_task(draft)]);
}

fn with(id: &str, files: &[&str], acceptance: Vec<CriterionDraft>) -> TaskDraft {
    TaskDraft {
        scope: TaskScope {
            files: files.iter().map(|file| file.to_string()).collect(),
            symbols: Vec::new(),
        },
        acceptance,
        ..task(id)
    }
}

fn requirement(state: &mut PlanState, id: &str) {
    submit(
        state,
        user(),
        vec![Proposal::AddItem {
            id: Some(EntryId::from(id)),
            kind: PlanItemKind::Requirement,
            text: format!("Requirement {id}"),
        }],
    );
}

fn tests_pass() -> Vec<CriterionDraft> {
    vec![check_criterion("tests pass", CheckKind::Test)]
}

#[test]
fn requirements_need_a_task_with_criteria() {
    let mut state = plan(PlanStatus::Active);
    requirement(&mut state, "r1");
    requirement(&mut state, "r2");
    requirement(&mut state, "r3");
    let serving = |id: &str, item: &str, acceptance| TaskDraft {
        requirements: vec![EntryId::from(item)],
        ..with(id, &[], acceptance)
    };
    add_task(&mut state, serving("a", "r1", vec![]));
    add_task(&mut state, serving("b", "r3", tests_pass()));
    let findings = run(&state);
    assert_eq!(
        ids(&findings),
        [
            "uncoveredRequirement:r1",
            "uncoveredRequirement:r2",
            "taskWithoutCriteria:a",
        ]
    );
    assert_eq!(findings[0].severity, AuditSeverity::High);
    assert!(findings[0].text.contains("no task with criteria"));
    assert_eq!(
        findings[0].refs,
        [
            AuditRef::Item {
                item_id: EntryId::from("r1")
            },
            AuditRef::Task { task_id: tid("a") },
        ]
    );
    assert!(findings[1].text.ends_with("has no task"));
    assert!(findings.iter().all(|f| f.status == AuditStatus::Open
        && f.source == AuditSource::Deterministic
        && f.suggestion.is_some()));
}

#[test]
fn only_leaf_tasks_need_their_own_criteria() {
    let mut state = plan(PlanStatus::Active);
    add_task(&mut state, with("p", &[], vec![]));
    add_task(
        &mut state,
        TaskDraft {
            parent: Some(tid("p")),
            ..with("c", &[], vec![])
        },
    );
    add_task(&mut state, with("done", &[], vec![]));
    submit(&mut state, user(), vec![set("done", TaskStatus::Cancelled)]);
    assert_eq!(ids(&run(&state)), ["taskWithoutCriteria:c"]);
}

#[test]
fn check_criteria_need_a_check_the_project_has() {
    let mut state = plan(PlanStatus::Active);
    add_task(
        &mut state,
        with(
            "a",
            &[],
            vec![check_criterion("lint clean", CheckKind::Lint)],
        ),
    );
    assert!(run(&state).is_empty());
    let criterion = state.task(&tid("a")).unwrap().acceptance[0].id.clone();
    let risky = risky();
    let findings = audit(
        &state,
        &AuditInput {
            check_kinds: Some(&[CheckKind::Test]),
            risky_paths: &risky,
        },
    );
    assert_eq!(
        ids(&findings),
        [format!("criterionWithoutMethod:a/{criterion}")]
    );
    assert!(findings[0].text.contains("lint"));
}

#[test]
fn risky_paths_need_a_test_or_migration_criterion() {
    let mut state = plan(PlanStatus::Active);
    let looked = || vec![criterion("reviewed", CriterionMethod::Inspection)];
    add_task(
        &mut state,
        with("store", &["crates/store/src/log.rs"], looked()),
    );
    add_task(&mut state, with("tested", &["db/001.sql"], tests_pass()));
    let migration = vec![criterion("Migration loads old logs", CriterionMethod::User)];
    add_task(
        &mut state,
        with("migrated", &["migrations/002/**"], migration),
    );
    add_task(&mut state, with("app", &["crates/app/**"], looked()));
    let findings = run(&state);
    assert_eq!(ids(&findings), ["riskyPathWithoutTest:store"]);
    assert_eq!(
        findings[0].refs,
        [
            AuditRef::Task {
                task_id: tid("store")
            },
            AuditRef::File {
                path: "**/store/**".into()
            },
        ]
    );
}

#[test]
fn risks_need_a_rollback_note() {
    let mut state = plan(PlanStatus::Active);
    let risky_notes = |rollback: Option<&str>| TaskNotes {
        risks: vec!["may drop rows".into()],
        rollback: rollback.map(String::from),
        ..TaskNotes::default()
    };
    for (id, rollback) in [
        ("bare", None),
        ("blank", Some("  ")),
        ("safe", Some("revert")),
    ] {
        let draft = TaskDraft {
            notes: risky_notes(rollback),
            ..with(id, &[], tests_pass())
        };
        add_task(&mut state, draft);
    }
    let findings = run(&state);
    assert_eq!(
        ids(&findings),
        ["riskWithoutMitigation:bare", "riskWithoutMitigation:blank"]
    );
    assert_eq!(findings[0].severity, AuditSeverity::Medium);
}

#[test]
fn overlapping_and_repeated_tasks_are_reported() {
    let mut state = plan(PlanStatus::Active);
    add_task(&mut state, with("a", &["src/api/**"], tests_pass()));
    add_task(&mut state, with("b", &["src/api/client.rs"], tests_pass()));
    let twin = |id: &str| TaskDraft {
        title: "Write docs".into(),
        ..with(id, &[], tests_pass())
    };
    add_task(&mut state, twin("d1"));
    add_task(&mut state, twin("d2"));
    let findings = run(&state);
    assert_eq!(ids(&findings), ["overlap:a+b", "duplicate:d1+d2"]);
    assert_eq!(findings[0].kind, AuditKind::Overlap);
    assert_eq!(findings[1].severity, AuditSeverity::Low);
}

#[test]
fn ids_survive_a_scope_edit() {
    let mut state = plan(PlanStatus::Active);
    add_task(&mut state, with("s", &["crates/store/a.rs"], vec![]));
    let before = ids(&run(&state));
    let patch = hyperform_protocol::work::TaskPatch {
        scope: Some(TaskScope {
            files: vec!["crates/store/b.rs".into()],
            symbols: Vec::new(),
        }),
        ..Default::default()
    };
    submit(
        &mut state,
        user(),
        vec![Proposal::EditTask {
            task_id: tid("s"),
            patch: Box::new(patch),
        }],
    );
    assert_eq!(ids(&run(&state)), before);
    assert_eq!(before, ["taskWithoutCriteria:s", "riskyPathWithoutTest:s"]);
}
