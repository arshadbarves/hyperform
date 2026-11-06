use z_engine_protocol::EntryId;
use z_engine_protocol::work::{
    ChangeCause, Discovery, EntryBody, PlanChange, PlanStatus, TaskPatch, WorkCause, WorkEvent,
    WorkRecord,
};

use super::*;
use crate::testkit::*;
use crate::{EntryDraft, PlanState, Proposal, Submission, decide};

fn record(id: &str, body: EntryBody) -> Proposal {
    Proposal::Record(Box::new(EntryDraft {
        id: Some(EntryId::from(id)),
        ..EntryDraft::new(body)
    }))
}

fn submit_caused(
    state: &mut PlanState,
    log: &mut Vec<WorkRecord>,
    cause: WorkCause,
    proposals: Vec<Proposal>,
) {
    let submission = Submission::new(main_agent(), cause, proposals);
    let records = decide(state, &submission, &facts()).unwrap();
    state.apply_all(&records);
    log.extend(records);
}

#[test]
fn why_follows_a_task_back_to_the_discovery_behind_its_change() {
    let mut state = plan(PlanStatus::Active);
    let mut log = Vec::new();
    submit_caused(&mut state, &mut log, cause(), vec![create_task(task("a"))]);
    submit_caused(&mut state, &mut log, cause(), vec![create_task(task("b"))]);
    let discovery = EntryBody::Discovery(Discovery {
        text: "the API is rate limited".into(),
        source: None,
    });
    let change = EntryBody::PlanChange(PlanChange {
        cause: ChangeCause::Discovery,
        source: Some(EntryId::from("ent_d")),
        original: "call per item".into(),
        new: "batch calls".into(),
        reason: "rate limits".into(),
        evidence: vec![],
        affected: vec![tid("a")],
    });
    submit_caused(
        &mut state,
        &mut log,
        cause(),
        vec![record("ent_d", discovery), record("ent_c", change)],
    );
    let rename = Proposal::EditTask {
        task_id: tid("a"),
        patch: Box::new(TaskPatch {
            title: Some("Batch the calls".into()),
            ..TaskPatch::default()
        }),
    };
    submit_caused(
        &mut state,
        &mut log,
        WorkCause::Change {
            entry_id: EntryId::from("ent_c"),
        },
        vec![rename],
    );
    let steps = why(&log, &state, &tid("a"));
    let kinds: Vec<&str> = steps
        .iter()
        .map(|step| match &step.event {
            WorkEvent::TaskCreated { .. } => "created",
            WorkEvent::EntryRecorded { .. } => "change",
            WorkEvent::TaskEdited { .. } => "edited",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["created", "change", "edited"]);
    let ids =
        |step: &WhyStep| -> Vec<String> { step.entries.iter().map(|e| e.id.to_string()).collect() };
    assert_eq!(ids(&steps[1]), ["ent_d"]);
    assert_eq!(ids(&steps[2]), ["ent_c", "ent_d"]);
    assert!(
        why(&log, &state, &tid("b"))
            .iter()
            .all(|step| step.entries.is_empty())
    );
}
