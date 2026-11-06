//! The plan audit: the kernel's deterministic findings reach the GUI with
//! each `WorkUpdate` and are recorded only when they change; the user's
//! `requestAudit` runs the read-only audit agent, whose JSON answer becomes
//! audit-agent findings (a malformed answer leaves a notice and records
//! nothing); `addFinding` turns a finding into a Pending task and tells the
//! main agent; a dismissed finding stays dismissed when the audit runs
//! again. The plan's depth is assessed again from all its tasks when
//! planning ends and when the user approves, so a plan that grew deep in
//! plan mode gets its automatic audit.

mod support;

use hyperform_protocol::work::{
    AuditFinding, AuditSource, AuditStatus, CriterionDraft, CriterionMethod, PlanDepth, PlanStatus,
    TaskStatus, WorkCause, WorkEdit, WorkEvent,
};
use hyperform_protocol::{Command, Event, PlanDecision};
use hyperform_testkit::{FixtureRepo, Script};
use serde_json::json;
use support::{Harness, last_user_text, new_task, task, work_settings};

/// The first line of the audit agent's system prompt.
const AUDIT_NEEDLE: &str = "You audit a work plan";
const WITHOUT_CRITERIA: &str = "taskWithoutCriteria:t1";

async fn planned() -> Harness {
    let repo = FixtureRepo::git(&[("src/a.rs", "fn a() {}\n")]);
    let mut h = Harness::builder(repo)
        .settings(&work_settings(""))
        .start()
        .await;
    let scope = json!({ "scope": ["src/a.rs"] });
    h.model
        .push(Script::tool("TaskCreate", new_task("Tidy a", scope)));
    h.model.push(Script::text("Planned."));
    h.settled_turn("tidy a").await;
    h
}

const CALLERS: &str = "Tidying a can break its callers.";
const DOCS: &str = "The docs of a go stale.";
const NAMES: &str = "The new names clash with b.";

/// The audit agent's answer with findings of `texts`.
fn audit_answer(texts: &[&str]) -> Script {
    let findings: Vec<_> = texts
        .iter()
        .map(|text| {
            json!({
                "severity": "high",
                "text": text,
                "suggestion": format!("Fix: {text}"),
                "refs": [{ "type": "task", "taskId": "t1" }, { "type": "task", "taskId": "t9" }]
            })
        })
        .collect();
    Script::text(&json!({ "findings": findings }).to_string())
}

fn agent_finding(h: &Harness, text: &str) -> Option<AuditFinding> {
    agent_findings(h).into_iter().find(|f| f.text == text)
}

fn audited_records(h: &Harness) -> usize {
    let records = h.plan_log().records;
    let audited = records
        .iter()
        .filter(|r| matches!(r.event, WorkEvent::Audited { .. }));
    audited.count()
}

fn agent_findings(h: &Harness) -> Vec<AuditFinding> {
    let plan = h.plan();
    let found = plan
        .audit
        .iter()
        .filter(|f| f.source == AuditSource::AuditAgent);
    found.cloned().collect()
}

async fn finding_until(h: &mut Harness, id: &str, status: AuditStatus) {
    h.plan_until(|plan| plan.finding(id).is_some_and(|f| f.status == status))
        .await;
}

#[tokio::test]
async fn deterministic_findings_reach_the_gui_and_are_recorded_when_they_change() {
    let mut h = planned().await;
    let shown = h.events.seen().iter().any(|event| {
        matches!(event, Event::WorkUpdated { update }
            if update.audit.iter().any(|f| f.id == WITHOUT_CRITERIA && f.status == AuditStatus::Open))
    });
    assert!(shown, "the finding is in a WorkUpdate");
    let recorded = audited_records(&h);

    h.edit_plan(WorkEdit::EditGoal {
        goal: "Tidy everything".into(),
    });
    h.plan_until(|plan| plan.header.goal == "Tidy everything")
        .await;
    assert_eq!(
        audited_records(&h),
        recorded,
        "unchanged findings are not recorded"
    );

    let criteria = vec![CriterionDraft {
        id: None,
        text: "The user reviewed it".into(),
        method: CriterionMethod::User,
    }];
    h.edit_plan(WorkEdit::SetCriteria {
        task_id: task("t1"),
        criteria,
    });
    finding_until(&mut h, WITHOUT_CRITERIA, AuditStatus::Resolved).await;
    assert_eq!(audited_records(&h), recorded + 1);
}

#[tokio::test]
async fn a_requested_audit_records_agent_findings_and_an_added_one_becomes_a_task() {
    let mut h = planned().await;
    let answers = vec![
        audit_answer(&[CALLERS, DOCS]),
        audit_answer(&[CALLERS, DOCS, NAMES]),
    ];
    h.model.route_queue_system(AUDIT_NEEDLE, answers);
    h.edit_plan(WorkEdit::RequestAudit);
    h.plan_until(|plan| {
        plan.audit
            .iter()
            .any(|f| f.source == AuditSource::AuditAgent)
    })
    .await;
    assert_eq!(agent_findings(&h).len(), 2);
    let callers = agent_finding(&h, CALLERS).unwrap();
    assert_eq!(callers.status, AuditStatus::Open);
    assert_eq!(callers.refs.len(), 1, "the unknown task t9 is dropped");
    let request = h
        .model
        .requests()
        .into_iter()
        .find(|r| r.system_text().contains(AUDIT_NEEDLE))
        .unwrap();
    assert!(
        request.tools.is_empty(),
        "the audit is one call without tools"
    );

    h.edit_plan(WorkEdit::AddFinding {
        finding_id: callers.id.clone(),
    });
    finding_until(&mut h, &callers.id, AuditStatus::Added).await;
    let added = h.plan().task(&task("t2")).unwrap().clone();
    assert_eq!(added.status, TaskStatus::Pending);
    assert_eq!(added.title, format!("Fix: {CALLERS}"));
    h.model.push(Script::text("I will work it in."));
    h.settled_turn("go on").await;
    let told = last_user_text(h.main_requests().last().unwrap());
    assert!(
        told.contains("The user added an audit finding to the plan as task t2"),
        "{told}"
    );

    let docs = agent_finding(&h, DOCS).unwrap();
    for id in [docs.id.as_str(), WITHOUT_CRITERIA] {
        h.edit_plan(WorkEdit::DismissFinding {
            finding_id: id.into(),
        });
        finding_until(&mut h, id, AuditStatus::Dismissed).await;
    }
    h.edit_plan(WorkEdit::RequestAudit);
    h.plan_until(|plan| plan.audit.iter().any(|f| f.text == NAMES))
        .await;
    assert_eq!(
        agent_finding(&h, CALLERS).unwrap().status,
        AuditStatus::Added
    );
    assert_eq!(
        agent_finding(&h, DOCS).unwrap().status,
        AuditStatus::Dismissed
    );
    let plan = h.plan();
    let kept = plan.finding(WITHOUT_CRITERIA).unwrap();
    assert_eq!(kept.status, AuditStatus::Dismissed);
}

#[tokio::test]
async fn a_malformed_audit_answer_leaves_a_notice_and_no_findings() {
    let mut h = planned().await;
    h.model
        .route_system(AUDIT_NEEDLE, Script::text("Looks fine to me."));
    h.edit_plan(WorkEdit::RequestAudit);
    let notice = h.notice("The plan audit did not finish").await;
    assert!(notice.contains("did not answer with JSON"), "{notice}");
    assert!(agent_findings(&h).is_empty());
}

/// A plan mode session whose first task is in one file (a Light plan),
/// then tasks in `dirs`.
async fn planned_in_plan_mode(dirs: &[&str]) -> Harness {
    let settings = work_settings("[permissions]\nplanning = true\n");
    let h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    for dir in dirs {
        let scope = json!({ "scope": [format!("{dir}/x.rs")] });
        let input = new_task(&format!("Change {dir}"), scope);
        h.model.push(Script::tool("TaskCreate", input));
    }
    h
}

fn depth_changes(h: &Harness) -> Vec<(PlanDepth, WorkCause)> {
    let records = h.plan_log().records;
    let changed = records.into_iter().filter_map(|r| match r.event {
        WorkEvent::DepthChanged { depth } => Some((depth, r.cause)),
        _ => None,
    });
    changed.collect()
}

#[tokio::test]
async fn a_plan_that_grew_deep_is_audited_when_planning_ends() {
    let mut h = planned_in_plan_mode(&["a", "b", "c", "d", "e"]).await;
    h.model.route_system(AUDIT_NEEDLE, audit_answer(&[CALLERS]));
    let outline = json!({ "plan": "1. Change a to e" });
    h.model.push(Script::tool("ExitPlanMode", outline));
    h.model.push(Script::text("Building."));
    h.submit("change a to e");
    let request_id = match h.wait(|e| matches!(e, Event::PlanProposed { .. })).await {
        Event::PlanProposed { request_id, .. } => request_id,
        other => panic!("{other:?}"),
    };
    let created = h.plan_log().records[0].event.clone();
    assert!(
        matches!(
            created,
            WorkEvent::PlanCreated {
                depth: PlanDepth::Light,
                ..
            }
        ),
        "{created:?}"
    );
    let exit = WorkCause::Tool {
        tool: "ExitPlanMode".into(),
        call_id: None,
    };
    assert_eq!(depth_changes(&h), [(PlanDepth::Deep, exit)]);
    h.plan_until(|plan| plan.audit.iter().any(|f| f.text == CALLERS))
        .await;

    h.send(Command::ResolvePlan {
        request_id,
        decision: PlanDecision::Approve {
            mode: None,
            edited_plan: None,
            handoff: false,
        },
    });
    h.turn_finished().await;
    assert_eq!(h.plan().header.status, PlanStatus::Active);
    assert_eq!(depth_changes(&h).len(), 1, "approval found the same depth");
}

#[tokio::test]
async fn approving_a_draft_records_its_reassessed_depth() {
    let mut h = planned_in_plan_mode(&["a", "a", "b"]).await;
    h.model.push(Script::text("Here are the tasks."));
    h.settled_turn("plan a and b").await;
    assert_eq!(h.plan().header.depth, PlanDepth::Light);
    h.edit_plan(WorkEdit::Approve);
    let plan = h
        .plan_until(|plan| plan.header.status == PlanStatus::Active)
        .await;
    assert_eq!(plan.header.depth, PlanDepth::Standard);
    assert_eq!(
        depth_changes(&h),
        [(PlanDepth::Standard, WorkCause::Approval)]
    );
    let audited = h.model.requests().into_iter();
    assert!(
        !audited
            .into_iter()
            .any(|r| r.system_text().contains(AUDIT_NEEDLE))
    );
}
