use hyperform_protocol::work::{
    Criterion, CriterionMethod, CriterionStatus, Discovery, Entry, EntryBody, EntryStatus,
    EvidenceRef, PacketEntry, PacketEvidence, PacketReason, PacketTaskLine, ParallelPolicy,
    PlanDepth, PlanHeader, PlanItem, PlanItemKind, PlanItemStatus, PlanStatus, Provenance,
    ReconcileFinding, Task, TaskKind, TaskNotes, TaskPacket, TaskPriority, TaskScope, TaskStatus,
};
use hyperform_protocol::{CheckKind, EntryId, PlanId, SessionId, TaskId};

use super::*;

fn header() -> PlanHeader {
    PlanHeader {
        plan_id: PlanId::from("pln_1"),
        project_root: "/p".into(),
        goal: "Ship retries".into(),
        document: String::new(),
        status: PlanStatus::Active,
        depth: PlanDepth::Standard,
        parallel: ParallelPolicy::default(),
        revision: 7,
        requirements: Vec::new(),
        invariants: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }
}

pub(super) fn task(id: &str, title: &str, status: TaskStatus) -> Task {
    Task {
        id: TaskId::from(id),
        parent: None,
        title: title.into(),
        active_form: String::new(),
        description: String::new(),
        kind: TaskKind::Implement,
        priority: TaskPriority::Normal,
        order: 0,
        status,
        held: false,
        run_alone: false,
        depends_on: Vec::new(),
        requirements: Vec::new(),
        scope: TaskScope::default(),
        notes: TaskNotes::default(),
        acceptance: Vec::new(),
        steps: Vec::new(),
        owner: None,
        result: None,
        resume: None,
        attempts: 0,
        run: Default::default(),
        provenance: Provenance::Agent,
        created_at: 1,
        updated_at: 1,
        created_revision: 1,
        revision: 1,
    }
}

fn packet() -> TaskPacket {
    let mut task = task("t2", "Add retry", TaskStatus::Active);
    task.acceptance = vec![Criterion {
        id: "c1".into(),
        text: "Retry tests pass".into(),
        method: CriterionMethod::Check {
            kind: CheckKind::Test,
        },
        status: CriterionStatus::Pending,
        evidence: Vec::new(),
    }];
    let check = EvidenceRef::Check {
        session_id: SessionId::from("s"),
        record_id: "chk_1".into(),
        passed: false,
        fingerprint: "f0".into(),
        kind: Some(CheckKind::Test),
        label: "cargo test".into(),
        at: 1,
    };
    TaskPacket {
        plan_id: PlanId::from("pln_1"),
        revision: 7,
        goal: "Ship retries".into(),
        invariants: vec![PlanItem {
            id: EntryId::from("e1"),
            kind: PlanItemKind::Invariant,
            text: "Public API unchanged".into(),
            provenance: Provenance::User,
            status: PlanItemStatus::Active,
            revision: 1,
        }],
        requirements: Vec::new(),
        task,
        parent: None,
        dependencies: Vec::new(),
        entries: vec![PacketEntry {
            entry: Entry {
                id: EntryId::from("e2"),
                body: EntryBody::Discovery(Discovery {
                    text: "429 is not retried".into(),
                    source: Some("src/client.rs".into()),
                }),
                provenance: Provenance::Compaction,
                status: EntryStatus::Proposed,
                tasks: vec![TaskId::from("t2")],
                files: Vec::new(),
                created_at: 1,
                created_revision: 1,
                revision: 1,
            },
            reason: PacketReason::Task,
            tokens: 10,
        }],
        evidence: vec![PacketEvidence {
            criterion_id: "c1".into(),
            evidence: check,
            stale: true,
        }],
        blockers: Vec::new(),
        scope: vec!["src/client.rs".into()],
        resume: None,
        next_ready: Some(PacketTaskLine {
            id: TaskId::from("t3"),
            title: "Document it".into(),
            status: TaskStatus::Pending,
        }),
        tokens: 120,
        budget: 4_000,
        dropped: 0,
    }
}

#[test]
fn the_packet_lists_what_the_task_needs_and_drops_empty_sections() {
    let text = packet_text(&packet());
    assert!(text.starts_with("Goal: Ship retries\n"), "{text}");
    assert!(text.contains("Invariants, which must stay true:\n- e1: Public API unchanged"));
    assert!(text.contains("Task t2: Add retry (in_progress)"));
    assert!(text.contains(
        "- c1 [pending] Retry tests pass (test check); evidence: test check `cargo test` failed, stale"
    ));
    assert!(text.contains("- e2: discovery: 429 is not retried [src/client.rs] (unconfirmed)"));
    assert!(text.contains("Next ready task: t3: Document it (pending)"));
    assert!(text.contains("Scope: src/client.rs"));
    for gone in [
        "Requirements:",
        "Parent:",
        "Depends on:",
        "Blocked:",
        "Steps",
        "left out",
    ] {
        assert!(!text.contains(gone), "{gone} should be dropped:\n{text}");
    }
}

#[test]
fn the_packet_reminder_wraps_the_body() {
    let text = work_packet(&packet());
    assert!(text.starts_with("<system-reminder>\nThe task packet for t2 at plan revision 7."));
    assert!(text.contains("Goal: Ship retries"));
    assert!(text.ends_with("</system-reminder>"));
}

#[test]
fn the_pin_names_the_current_and_next_task() {
    let header = header();
    let current = task("t2", "Add retry", TaskStatus::Active);
    let pin = work_pin(&WorkPin {
        header: &header,
        current: Some(&current),
        next: None,
    });
    assert!(
        pin.contains("\"Ship retries\" (active, revision 7)"),
        "{pin}"
    );
    assert!(pin.contains("Current task: t2: Add retry"));
    assert!(!pin.contains("Next ready task"));
}

#[test]
fn findings_render_one_line_each() {
    let findings = [
        ReconcileFinding::ScopeDeleted {
            task_id: TaskId::from("t1"),
            files: vec!["a.rs".into()],
        },
        ReconcileFinding::DependencyFailed {
            task_id: TaskId::from("t3"),
            depends_on: TaskId::from("t2"),
        },
    ];
    let text = work_reconcile(&findings);
    assert!(
        text.contains(
            "- t1: files in its scope were deleted: a.rs\n- t3 depends on t2, which failed"
        )
    );
    assert!(!text.contains("{{"));
}

#[test]
fn the_task_list_groups_tasks_and_reports_health() {
    let header = header();
    let active = task("t1", "Parse", TaskStatus::Active);
    let mut waiting = task("t2", "Retry", TaskStatus::Pending);
    waiting.depends_on = vec![TaskId::from("t1")];
    let text = task_list(&TaskListView {
        header: &header,
        active: vec![&active],
        ready: Vec::new(),
        waiting: vec![&waiting],
        proposed: Vec::new(),
        closed: Vec::new(),
        next: None,
        findings: &[],
        health: TaskHealth {
            tasks_done: 0,
            tasks: 2,
            criteria_met: 1,
            criteria: 3,
            requirements_covered: 0,
            requirements: 0,
        },
    });
    assert!(
        text.contains("Active:\n- t1: Parse (in_progress)"),
        "{text}"
    );
    assert!(text.contains("Blocked or waiting:\n- t2: Retry (pending), after t1"));
    assert!(text.contains("Health: tasks done 0/2 · criteria met 1/3"));
    assert!(!text.contains("Ready:") && !text.contains("Closed:"));
}

#[test]
fn a_helper_brief_holds_the_packet_the_contract_and_the_prompt() {
    let text = helper_brief(&packet(), "  Add backoff to the client. ");
    assert!(text.contains("dispatched for task t2"), "{text}");
    assert!(text.contains("Retry tests pass"));
    assert!(text.contains("\"taskId\": \"t2\", \"report\""));
    assert!(text.ends_with("Add backoff to the client."));
    assert!(!text.contains("{{"));
}

#[test]
fn the_coordinator_hears_the_report_and_what_is_ready() {
    let mut done = task("t1", "Parse", TaskStatus::Verifying);
    done.run.report = Some(hyperform_protocol::work::TaskReport {
        summary: "Parser\nrewritten".into(),
        ..Default::default()
    });
    let (ready, held) = (
        task("t2", "Retry", TaskStatus::Pending),
        task("t3", "Docs", TaskStatus::Pending),
    );
    let news = HelperNews {
        agent: "agt_1",
        task: &done,
        event: "its change was applied",
        ready: &[&ready],
        parallel: &[],
        held: &[&held],
    };
    let text = work_coordinate(&news);
    assert!(text.contains("Helper agt_1 on task t1: its change was applied."));
    assert!(text.contains("Its report: Parser rewritten"), "{text}");
    assert!(text.contains("Newly ready tasks:\n- t2: Retry"));
    assert!(!text.contains("in parallel now:"));
    assert!(
        text.contains("plan change is accepted:\n- t3: Docs"),
        "{text}"
    );
}

#[test]
fn an_added_finding_names_its_task_and_refs() {
    use hyperform_protocol::work::{AuditFinding, AuditKind, AuditRef, AuditSeverity};
    let finding = AuditFinding {
        id: "review-1".into(),
        kind: AuditKind::ModelReview,
        severity: AuditSeverity::High,
        text: "No migration".into(),
        refs: vec![AuditRef::Task {
            task_id: TaskId::from("t1"),
        }],
        suggestion: None,
        status: Default::default(),
        source: Default::default(),
        created_revision: 1,
        revision: 1,
    };
    let text = work_finding_added(&finding, &TaskId::from("t4"));
    assert!(text.contains("as task t4: No migration"));
    assert!(text.contains("About: task t1"));
    assert!(!text.contains("Suggested change"));
    assert!(audit_request("{}").contains("```json\n{}\n```"));
}

#[test]
fn a_missing_report_reads_as_the_plan_tab_says_it() {
    let reason = hyperform_protocol::work::BlockReason::Unreported;
    let status = TaskStatus::Blocked { reason };
    assert_eq!(
        task_status(&status),
        "blocked: its agent ended without a report"
    );
}
