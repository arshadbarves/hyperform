use z_engine_protocol::work::{CriterionDraft, CriterionMethod, PlanStatus};
use z_engine_protocol::{CheckKind, PlanId, TaskId};
use z_engine_work::{Facts, PlanDraft, TaskDraft};

use super::*;

fn plan() -> PlanState {
    let draft = PlanDraft {
        plan_id: PlanId::new(),
        project_root: "/p".into(),
        goal: "Ship retries".into(),
        document: String::new(),
        depth: PlanDepth::Deep,
        status: PlanStatus::Draft,
    };
    let task = TaskDraft {
        id: Some(TaskId("t1".into())),
        title: "Parse".into(),
        acceptance: vec![CriterionDraft {
            id: None,
            text: "Parser tests pass".into(),
            method: CriterionMethod::Check {
                kind: CheckKind::Test,
            },
        }],
        ..TaskDraft::default()
    };
    let submission = Submission::new(
        Actor::User,
        WorkCause::Edit,
        vec![Proposal::CreateTask(Box::new(task))],
    );
    let records = z_engine_work::create(&draft, &submission, &Facts::at(1)).unwrap();
    PlanState::fold(&records).unwrap()
}

#[test]
fn findings_keep_known_refs_and_default_to_medium() {
    let state = plan();
    let criterion = state.task(&TaskId("t1".into())).unwrap().acceptance[0]
        .id
        .clone();
    let answer = format!(
        "Here is my review.\n```json\n{{\"findings\": [\
         {{\"text\": \" No migration step \", \"suggestion\": \"Add a migration task\", \"refs\": [\
         {{\"type\": \"task\", \"taskId\": \"t1\"}}, {{\"type\": \"task\", \"taskId\": \"t9\"}},\
         {{\"type\": \"criterion\", \"taskId\": \"t1\", \"criterionId\": \"{criterion}\"}},\
         {{\"type\": \"nonsense\"}}]}},\
         {{\"severity\": \"high\", \"text\": \"  \"}}]}}\n```"
    );
    let found = parse(&state, &answer).unwrap();
    assert_eq!(found.len(), 1, "a finding without text is dropped");
    let finding = &found[0];
    assert_eq!(finding.text, "No migration step");
    assert_eq!(finding.severity, AuditSeverity::Medium);
    assert_eq!(finding.kind, AuditKind::ModelReview);
    assert_eq!(finding.suggestion.as_deref(), Some("Add a migration task"));
    assert_eq!(finding.refs.len(), 2, "{:?}", finding.refs);
    assert!(finding.id.is_empty(), "the kernel numbers new reviews");
}

#[test]
fn an_answer_without_json_is_an_error() {
    let state = plan();
    assert!(parse(&state, "Looks fine to me.").is_err());
    assert!(parse(&state, "{not json}").is_err());
    assert!(parse(&state, "{\"findings\": []}").unwrap().is_empty());
}

#[test]
fn the_plan_json_names_tasks_and_criteria() {
    let text = plan_json(&plan());
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["goal"], "Ship retries");
    assert_eq!(value["tasks"][0]["id"], "t1");
    assert_eq!(
        value["tasks"][0]["acceptance"][0]["text"],
        "Parser tests pass"
    );
    assert!(same("No  migration", "no migration"));
}
