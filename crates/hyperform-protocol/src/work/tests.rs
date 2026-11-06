use serde_json::json;

use super::*;
use crate::ids::{AgentId, EntryId, PlanId, SessionId, TaskId};
use crate::verification::CheckKind;

#[test]
fn work_ids_are_prefixed() {
    assert!(PlanId::new().as_str().starts_with("pln_"));
    assert!(TaskId::new().as_str().starts_with("tsk_"));
    assert!(EntryId::new().as_str().starts_with("ent_"));
}

#[test]
fn task_status_is_tagged_by_type() {
    let blocked = TaskStatus::Blocked {
        reason: BlockReason::Dependency {
            task_id: TaskId::from("tsk_a"),
        },
    };
    let json = serde_json::to_value(&blocked).unwrap();
    assert_eq!(
        json,
        json!({"type": "blocked", "reason": {"type": "dependency", "taskId": "tsk_a"}})
    );
    assert_eq!(serde_json::from_value::<TaskStatus>(json).unwrap(), blocked);
    assert_eq!(
        serde_json::to_value(TaskStatus::Done).unwrap(),
        json!({"type": "done"})
    );
    let failed: TaskStatus = serde_json::from_value(json!({"type": "failed"})).unwrap();
    assert_eq!(failed, TaskStatus::Failed { reason: "".into() });
}

#[test]
fn tasks_logged_before_provenance_load_as_agent_work() {
    let json = json!({
        "id": "tsk_a", "parent": null, "title": "A", "activeForm": "", "description": "",
        "kind": "implement", "priority": "normal", "order": 0, "status": {"type": "pending"},
        "held": false, "runAlone": false, "dependsOn": [], "requirements": [],
        "scope": {"files": []}, "notes": {}, "acceptance": [], "steps": [], "owner": null,
        "result": null, "resume": null, "attempts": 0, "createdAt": 1, "updatedAt": 1,
        "createdRevision": 1, "revision": 1
    });
    let task: Task = serde_json::from_value(json).unwrap();
    assert_eq!(task.provenance, Provenance::Agent);
}

#[test]
fn actors_and_causes_use_camel_case() {
    let actor = Actor::Agent {
        session_id: SessionId::from("S"),
        agent_id: AgentId::main(),
    };
    assert_eq!(
        serde_json::to_value(&actor).unwrap(),
        json!({"type": "agent", "sessionId": "S", "agentId": "main"})
    );
    let cause = WorkCause::Tool {
        tool: "TaskUpdate".into(),
        call_id: None,
    };
    assert_eq!(
        serde_json::to_value(&cause).unwrap(),
        json!({"type": "tool", "tool": "TaskUpdate", "callId": null})
    );
}

#[test]
fn entry_bodies_merge_their_fields_under_the_tag() {
    let entry = Entry {
        id: EntryId::from("ent_1"),
        body: EntryBody::Discovery(Discovery {
            text: "C is generated".into(),
            source: Some("src/c.rs".into()),
        }),
        provenance: Provenance::Compaction,
        status: EntryStatus::Proposed,
        tasks: vec![],
        files: vec!["src/c.rs".into()],
        created_at: 1,
        created_revision: 2,
        revision: 2,
    };
    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["body"]["type"], "discovery");
    assert_eq!(json["body"]["text"], "C is generated");
    assert_eq!(json["status"], json!({"type": "proposed"}));
    assert_eq!(json["provenance"], "compaction");
    assert_eq!(serde_json::from_value::<Entry>(json).unwrap(), entry);
}

#[test]
fn work_records_round_trip() {
    let record = WorkRecord {
        seq: 4,
        revision: 3,
        at: 1_700,
        actor: Actor::Kernel,
        cause: WorkCause::Change {
            entry_id: EntryId::from("ent_c"),
        },
        event: WorkEvent::EvidenceLinked {
            task_id: TaskId::from("tsk_a"),
            criterion_id: Some("c1".into()),
            evidence: EvidenceRef::Check {
                session_id: SessionId::from("S"),
                record_id: "chk_1".into(),
                passed: true,
                fingerprint: "fp".into(),
                kind: Some(CheckKind::Test),
                label: "cargo test".into(),
                at: 9,
            },
        },
    };
    let json = serde_json::to_value(&record).unwrap();
    assert_eq!(json["event"]["type"], "evidenceLinked");
    assert_eq!(json["event"]["criterionId"], "c1");
    assert_eq!(json["event"]["evidence"]["recordId"], "chk_1");
    assert_eq!(json["cause"]["entryId"], "ent_c");
    assert_eq!(serde_json::from_value::<WorkRecord>(json).unwrap(), record);
}

#[test]
fn gui_edits_deserialize() {
    let edit: WorkEdit = serde_json::from_value(json!({
        "type": "setCriteria",
        "taskId": "tsk_a",
        "criteria": [{"text": "tests pass", "method": {"type": "check", "kind": "test"}}]
    }))
    .unwrap();
    let WorkEdit::SetCriteria { criteria, .. } = edit else {
        panic!("wrong variant");
    };
    assert_eq!(criteria[0].id, None);
    assert_eq!(
        criteria[0].method,
        CriterionMethod::Check {
            kind: CheckKind::Test
        }
    );
    let approve: WorkEdit = serde_json::from_value(json!({"type": "approve"})).unwrap();
    assert_eq!(approve, WorkEdit::Approve);
    let patch: WorkEdit = serde_json::from_value(json!({
        "type": "editTask", "taskId": "tsk_a", "patch": {"title": "New"}
    }))
    .unwrap();
    let WorkEdit::EditTask { patch, .. } = patch else {
        panic!("wrong variant");
    };
    assert_eq!(patch.title.as_deref(), Some("New"));
    assert_eq!(patch.scope, None);
}

#[test]
fn session_wire_variants_for_work_round_trip() {
    use crate::{Command, Event};
    let sent = Event::TaskPacketSent {
        agent_id: AgentId::main(),
        task_id: TaskId::from("tsk_a"),
        tokens: 812,
        reason: "compaction".into(),
    };
    let json = serde_json::to_value(&sent).unwrap();
    assert_eq!(
        json,
        json!({"type": "taskPacketSent", "agentId": "main", "taskId": "tsk_a",
               "tokens": 812, "reason": "compaction"})
    );
    assert_eq!(serde_json::from_value::<Event>(json).unwrap(), sent);

    let edit: Command = serde_json::from_value(json!({
        "type": "editWork", "planId": "pln_1", "edit": {"type": "holdTask", "taskId": "tsk_a", "held": true}
    }))
    .unwrap();
    assert_eq!(
        edit,
        Command::EditWork {
            plan_id: PlanId::from("pln_1"),
            edit: WorkEdit::HoldTask {
                task_id: TaskId::from("tsk_a"),
                held: true
            }
        }
    );
}

#[test]
fn work_updated_carries_the_update_inline() {
    let plan = PlanHeader {
        plan_id: PlanId::from("pln_1"),
        project_root: "/p".into(),
        goal: "Ship".into(),
        document: String::new(),
        status: PlanStatus::Active,
        depth: PlanDepth::Light,
        parallel: ParallelPolicy::Off,
        revision: 7,
        requirements: vec![],
        invariants: vec![],
        created_at: 1,
        updated_at: 2,
    };
    let event = crate::Event::WorkUpdated {
        update: Box::new(WorkUpdate {
            plan_id: plan.plan_id.clone(),
            revision: 7,
            plan,
            tasks: vec![],
            entries: vec![],
            change: None,
            audit: vec![],
            findings: vec![],
            health: PlanHealth::default(),
            replan_held: vec![TaskId::from("tsk_a")],
        }),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "workUpdated");
    assert_eq!(json["update"]["planId"], "pln_1");
    assert_eq!(json["update"]["replanHeld"], json!(["tsk_a"]));
    assert_eq!(json["update"]["plan"]["status"], "active");
    assert_eq!(json["update"]["health"]["evidence"], "fair");
    assert_eq!(serde_json::from_value::<crate::Event>(json).unwrap(), event);
}

#[test]
fn parallel_edits_and_events_use_camel_case() {
    let edits = [
        (
            json!({"type": "approveParallel"}),
            WorkEdit::ApproveParallel,
        ),
        (
            json!({"type": "retrySerially", "taskId": "tsk_a"}),
            WorkEdit::RetrySerially {
                task_id: TaskId::from("tsk_a"),
            },
        ),
        (
            json!({"type": "dismissFinding", "findingId": "overlap:a+b"}),
            WorkEdit::DismissFinding {
                finding_id: "overlap:a+b".into(),
            },
        ),
        (json!({"type": "requestAudit"}), WorkEdit::RequestAudit),
        (
            json!({"type": "recheckTask", "taskId": "tsk_a"}),
            WorkEdit::RecheckTask {
                task_id: TaskId::from("tsk_a"),
            },
        ),
    ];
    for (json, edit) in edits {
        assert_eq!(serde_json::from_value::<WorkEdit>(json).unwrap(), edit);
    }
    let retried: WorkEvent =
        serde_json::from_value(json!({"type": "taskRetried", "taskId": "tsk_a"})).unwrap();
    assert_eq!(
        retried,
        WorkEvent::TaskRetried {
            task_id: TaskId::from("tsk_a"),
            serial: false,
            fresh_attempts: false
        }
    );
    let changed = WorkEvent::FindingStatusChanged {
        finding_id: "f".into(),
        status: AuditStatus::Added,
    };
    assert_eq!(
        serde_json::to_value(&changed).unwrap(),
        json!({"type": "findingStatusChanged", "findingId": "f", "status": "added"})
    );
    let deeper = WorkEvent::DepthChanged {
        depth: PlanDepth::Deep,
    };
    assert_eq!(
        serde_json::to_value(&deeper).unwrap(),
        json!({"type": "depthChanged", "depth": "deep"})
    );
}

#[test]
fn finding_tasks_name_every_task_involved() {
    let finding = ReconcileFinding::DependencySuperseded {
        task_id: TaskId::from("a"),
        depends_on: TaskId::from("b"),
        by: TaskId::from("c"),
    };
    assert_eq!(finding.tasks().len(), 2);
    let json = serde_json::to_value(&finding).unwrap();
    assert_eq!(json["type"], "dependencySuperseded");
    assert_eq!(json["dependsOn"], "b");
}
