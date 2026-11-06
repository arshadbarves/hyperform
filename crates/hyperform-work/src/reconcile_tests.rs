use hyperform_protocol::verification::CheckKind;
use hyperform_protocol::work::{Actor, PlanStatus, ReconcileFinding, TaskScope, TaskStatus};

use super::*;
use crate::testkit::*;
use crate::{Proposal, TaskDraft};

fn scoped(id: &str, files: &[&str], deps: &[&str]) -> Proposal {
    create_task(TaskDraft {
        scope: TaskScope {
            files: files.iter().map(|f| f.to_string()).collect(),
            symbols: vec![],
        },
        depends_on: deps.iter().map(|d| tid(d)).collect(),
        ..task(id)
    })
}

fn files(items: &[&str]) -> std::collections::BTreeSet<String> {
    items.iter().map(|f| f.to_string()).collect()
}

#[test]
fn a_consistent_plan_has_no_findings() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![
            scoped("a", &["src/a.rs"], &[]),
            scoped("b", &["src/b.rs"], &["a"]),
        ],
    );
    let facts = Facts {
        changed_files: files(&["README.md"]),
        ..facts()
    };
    assert!(findings(&state, &facts).is_empty());
}

#[test]
fn deleted_scope_files_are_reported_for_open_tasks() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![
            scoped("a", &["src/auth/**"], &[]),
            scoped("b", &["src/auth/x.rs"], &[]),
        ],
    );
    submit(&mut state, user(), vec![set("b", TaskStatus::Cancelled)]);
    let facts = Facts {
        deleted_files: files(&["src/auth/login.rs", "src/auth/x.rs", "docs/a.md"]),
        ..facts()
    };
    assert_eq!(
        findings(&state, &facts),
        [ReconcileFinding::ScopeDeleted {
            task_id: tid("a"),
            files: vec!["src/auth/login.rs".into(), "src/auth/x.rs".into()],
        }]
    );
}

#[test]
fn done_work_whose_files_changed_or_checks_went_stale_is_reported() {
    let mut state = plan(PlanStatus::Active);
    let draft = TaskDraft {
        acceptance: vec![check_criterion("tests pass", CheckKind::Test)],
        scope: TaskScope {
            files: vec!["src/lib.rs".into()],
            symbols: vec![],
        },
        ..task("a")
    };
    submit(&mut state, user(), vec![create_task(draft)]);
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r1", true, FP))],
    );
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    let facts = Facts {
        changed_files: files(&["src/lib.rs"]),
        stale_records: files(&["r1"]),
        ..facts()
    };
    assert_eq!(
        findings(&state, &facts),
        [ReconcileFinding::StaleEvidence {
            task_id: tid("a"),
            files: vec!["src/lib.rs".into()],
            records: vec!["r1".into()],
        }]
    );
}

#[test]
fn changes_counted_from_before_a_task_was_done_may_be_its_own() {
    let mut state = plan(PlanStatus::Active);
    submit(&mut state, user(), vec![scoped("a", &["src/lib.rs"], &[])]);
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    let changed_since = |since: u64| Facts {
        changed_files: files(&["src/lib.rs"]),
        changed_since: Some(since),
        ..facts()
    };
    assert!(findings(&state, &changed_since(1_000)).is_empty());
    assert_eq!(
        findings(&state, &changed_since(1_001)),
        [ReconcileFinding::StaleEvidence {
            task_id: tid("a"),
            files: vec!["src/lib.rs".into()],
            records: vec![],
        }]
    );
}

#[test]
fn a_check_that_fails_after_the_task_is_done_is_stale_evidence() {
    let mut state = plan(PlanStatus::Active);
    let draft = TaskDraft {
        acceptance: vec![check_criterion("tests pass", CheckKind::Test)],
        ..task("a")
    };
    submit(&mut state, user(), vec![create_task(draft)]);
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r1", true, FP))],
    );
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Done)]);
    submit(
        &mut state,
        Actor::Kernel,
        vec![link("a", "c1", check("r2", false, FP))],
    );
    assert_eq!(status_of(&state, "a"), TaskStatus::Done);
    assert_eq!(
        findings(&state, &facts()),
        [ReconcileFinding::StaleEvidence {
            task_id: tid("a"),
            files: vec![],
            records: vec!["r2".into()],
        }]
    );
}

#[test]
fn failed_and_superseded_dependencies_are_reported() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![
            scoped("a", &[], &[]),
            scoped("s", &[], &[]),
            scoped("n", &[], &[]),
            scoped("b", &[], &["a", "s"]),
        ],
    );
    submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
    submit(
        &mut state,
        main_agent(),
        vec![
            set("a", TaskStatus::Failed { reason: "x".into() }),
            set("s", TaskStatus::Superseded { by: tid("n") }),
        ],
    );
    assert_eq!(
        findings(&state, &facts()),
        [
            ReconcileFinding::DependencyFailed {
                task_id: tid("b"),
                depends_on: tid("a"),
            },
            ReconcileFinding::DependencySuperseded {
                task_id: tid("b"),
                depends_on: tid("s"),
                by: tid("n"),
            },
        ]
    );
}

#[test]
fn unordered_open_tasks_with_overlapping_scopes_are_reported() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![
            scoped("a", &["src/api/**"], &[]),
            scoped("b", &["src/api/routes.rs"], &[]),
            scoped("c", &["src/api/routes.rs"], &["b"]),
            scoped("d", &["src/ui/*.ts"], &[]),
        ],
    );
    assert_eq!(
        findings(&state, &facts()),
        [
            ReconcileFinding::Overlap {
                tasks: vec![tid("a"), tid("b")],
                files: vec!["src/api/routes.rs".into()],
            },
            ReconcileFinding::Overlap {
                tasks: vec![tid("a"), tid("c")],
                files: vec!["src/api/routes.rs".into()],
            },
        ]
    );
}

#[test]
fn open_tasks_with_the_same_title_are_duplicates() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![
            create_task(TaskDraft::new("Add login form").with_id("a")),
            create_task(TaskDraft::new("add  login-form").with_id("b")),
            create_task(TaskDraft::new("Add logout").with_id("c")),
        ],
    );
    assert_eq!(
        findings(&state, &facts()),
        [ReconcileFinding::Duplicate {
            tasks: vec![tid("a"), tid("b")],
            title: "Add login form".into(),
        }]
    );
    assert_eq!(normalize_title("  Add, LOGIN form! "), "add login form");
}

trait WithId {
    fn with_id(self, id: &str) -> Self;
}

impl WithId for TaskDraft {
    fn with_id(self, id: &str) -> Self {
        TaskDraft {
            id: Some(tid(id)),
            ..self
        }
    }
}
