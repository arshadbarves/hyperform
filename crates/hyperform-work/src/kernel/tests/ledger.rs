use hyperform_protocol::EntryId;
use hyperform_protocol::verification::CheckKind;
use hyperform_protocol::work::{
    Actor, ChangeCause, DesignDecision, Discovery, EntryBody, EntryStatus, EvidenceRef, Experiment,
    ExperimentOutcome, PlanChange, PlanStatus, Provenance, TaskStatus, WorkEdit,
};

use crate::testkit::*;
use crate::{EntryDraft, Facts, PlanState, Proposal, Reason, TaskDraft};

fn eid(id: &str) -> EntryId {
    EntryId::from(id)
}

fn record(id: &str, body: EntryBody) -> Proposal {
    Proposal::Record(Box::new(EntryDraft {
        id: Some(eid(id)),
        ..EntryDraft::new(body)
    }))
}

fn discovery(text: &str) -> EntryBody {
    EntryBody::Discovery(Discovery {
        text: text.into(),
        source: Some("https://example.com".into()),
    })
}

fn change(source: Option<&str>, reason: &str) -> EntryBody {
    EntryBody::PlanChange(PlanChange {
        cause: ChangeCause::Discovery,
        source: source.map(eid),
        original: "use the cache".into(),
        new: "skip the cache".into(),
        reason: reason.into(),
        evidence: vec![],
        affected: vec![tid("a")],
    })
}

fn entry_status(state: &PlanState, id: &str) -> EntryStatus {
    state.entry(&eid(id)).unwrap().status.clone()
}

#[test]
fn untrusted_discoveries_are_recorded_unconfirmed() {
    let mut state = plan(PlanStatus::Active);
    let research = Facts {
        untrusted_input: true,
        ..facts()
    };
    submit_with(
        &mut state,
        main_agent(),
        vec![record("ent_d", discovery("caps at 50MB"))],
        &research,
    );
    let entry = state.entry(&eid("ent_d")).unwrap();
    assert_eq!(
        (entry.status.clone(), entry.provenance),
        (EntryStatus::Proposed, Provenance::Research)
    );
    let confirm = Proposal::from(WorkEdit::ConfirmEntry {
        entry_id: eid("ent_d"),
    });
    assert!(refuse(&state, main_agent(), vec![confirm.clone()]).needs_user());
    submit(&mut state, user(), vec![confirm]);
    assert_eq!(entry_status(&state, "ent_d"), EntryStatus::Accepted);
    let decision = EntryBody::DesignDecision(DesignDecision {
        title: "Cache".into(),
        choice: "none".into(),
        alternatives: vec![],
        reason: "the page said so".into(),
    });
    let experiment = EntryBody::Experiment(Experiment {
        hypothesis: "the cap is per file".into(),
        method: "upload two files".into(),
        result: None,
        conclusion: None,
        outcome: ExperimentOutcome::Open,
    });
    submit_with(
        &mut state,
        main_agent(),
        vec![record("ent_x", decision), record("ent_e", experiment)],
        &research,
    );
    for id in ["ent_x", "ent_e"] {
        let entry = state.entry(&eid(id)).unwrap();
        assert_eq!(
            (entry.status.clone(), entry.provenance),
            (EntryStatus::Proposed, Provenance::Research)
        );
    }
}

#[test]
fn compaction_may_only_propose_discoveries() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        Actor::Compaction,
        vec![record("ent_c", discovery("summarized"))],
    );
    let entry = state.entry(&eid("ent_c")).unwrap();
    assert_eq!(
        (entry.status.clone(), entry.provenance),
        (EntryStatus::Proposed, Provenance::Compaction)
    );
    assert!(matches!(
        refuse(&state, Actor::Compaction, vec![create_task(task("a"))]).reasons[0],
        Reason::Compaction(_)
    ));
}

#[test]
fn entries_must_reference_what_exists() {
    let mut state = plan(PlanStatus::Active);
    let linked = Proposal::Record(Box::new(EntryDraft {
        tasks: vec![tid("ghost")],
        ..EntryDraft::new(discovery("x"))
    }));
    assert_eq!(
        refuse(&state, main_agent(), vec![linked]).reasons,
        [Reason::UnknownTask(tid("ghost"))]
    );
    add(&mut state, "a", &[]);
    assert_eq!(
        refuse(
            &state,
            main_agent(),
            vec![record("ent_p", change(Some("ent_none"), "why"))]
        )
        .reasons,
        [Reason::UnknownEntry(eid("ent_none"))]
    );
    assert!(
        refuse(
            &state,
            main_agent(),
            vec![record("ent_p", change(None, " "))]
        )
        .to_string()
        .contains("needs a reason")
    );
    submit(
        &mut state,
        main_agent(),
        vec![record("ent_d", discovery("x"))],
    );
    submit(
        &mut state,
        main_agent(),
        vec![record("ent_p", change(Some("ent_d"), "measured"))],
    );
    assert_eq!(state.entry(&eid("ent_p")).unwrap().tasks, vec![tid("a")]);
    assert!(matches!(
        refuse(
            &state,
            main_agent(),
            vec![record("ent_d", discovery("again"))]
        )
        .reasons[0],
        Reason::DuplicateId(_)
    ));
}

#[test]
fn check_evidence_is_linked_by_the_engine_and_judges_the_criterion() {
    let mut state = plan(PlanStatus::Active);
    let draft = TaskDraft {
        acceptance: vec![check_criterion("tests pass", CheckKind::Test)],
        ..task("a")
    };
    submit(&mut state, user(), vec![create_task(draft)]);
    let failing = link("a", "c1", check("r1", false, FP));
    assert!(matches!(
        refuse(&state, main_agent(), vec![failing.clone()]).reasons[0],
        Reason::EngineOnly(_)
    ));
    let task_level = Proposal::LinkEvidence {
        task_id: tid("a"),
        criterion_id: None,
        evidence: check("r1", true, FP),
    };
    assert!(matches!(
        refuse(&state, main_agent(), vec![task_level]).reasons[0],
        Reason::EngineOnly(_)
    ));
    let records = submit(&mut state, Actor::Kernel, vec![failing]);
    assert_eq!(records.len(), 2);
    let criterion = &state.task(&tid("a")).unwrap().acceptance[0];
    assert_eq!(
        (criterion.evidence.len(), criterion.status),
        (1, hyperform_protocol::work::CriterionStatus::Failed)
    );
    assert!(matches!(
        refuse(
            &state,
            Actor::Kernel,
            vec![link("a", "c9", check("r2", true, FP))]
        )
        .reasons[0],
        Reason::UnknownCriterion { .. }
    ));
}

#[test]
fn notes_and_diffs_for_the_whole_task_become_evidence_entries() {
    let mut state = plan(PlanStatus::Active);
    add(&mut state, "a", &[]);
    let diff = EvidenceRef::Diff {
        files: vec!["src/lib.rs".into()],
        checkpoint: None,
        summary: "+10 -2".into(),
    };
    submit(
        &mut state,
        main_agent(),
        vec![Proposal::LinkEvidence {
            task_id: tid("a"),
            criterion_id: None,
            evidence: diff,
        }],
    );
    let entry = state.entries.values().next().unwrap();
    assert!(matches!(entry.body, EntryBody::Evidence(_)));
    assert_eq!(
        (entry.tasks.clone(), entry.files.clone()),
        (vec![tid("a")], vec!["src/lib.rs".to_string()])
    );
    assert_eq!(status_of(&state, "a"), TaskStatus::Pending);
}

#[test]
fn experiments_conclude_with_an_outcome() {
    let mut state = plan(PlanStatus::Active);
    let experiment = EntryBody::Experiment(Experiment {
        hypothesis: "batching halves latency".into(),
        method: "bench".into(),
        result: None,
        conclusion: None,
        outcome: ExperimentOutcome::Open,
    });
    submit(
        &mut state,
        main_agent(),
        vec![record("ent_e", experiment), record("ent_d", discovery("x"))],
    );
    let conclude = |id: &str, outcome| Proposal::ConcludeExperiment {
        entry_id: eid(id),
        outcome,
        result: Some("-48%".into()),
        conclusion: Some("batch".into()),
    };
    assert!(
        refuse(
            &state,
            main_agent(),
            vec![conclude("ent_e", ExperimentOutcome::Open)]
        )
        .to_string()
        .contains("needs an outcome")
    );
    assert!(
        refuse(
            &state,
            main_agent(),
            vec![conclude("ent_d", ExperimentOutcome::Confirmed)]
        )
        .to_string()
        .contains("not an experiment")
    );
    submit(
        &mut state,
        main_agent(),
        vec![conclude("ent_e", ExperimentOutcome::Confirmed)],
    );
    let EntryBody::Experiment(done) = &state.entry(&eid("ent_e")).unwrap().body else {
        panic!("experiment");
    };
    assert_eq!(
        (done.outcome, done.result.as_deref()),
        (ExperimentOutcome::Confirmed, Some("-48%"))
    );
}

#[test]
fn dismissing_and_superseding_respect_who_recorded_the_entry() {
    let mut state = plan(PlanStatus::Active);
    submit(
        &mut state,
        user(),
        vec![record("ent_u", discovery("user note"))],
    );
    submit(
        &mut state,
        main_agent(),
        vec![
            record("ent_a", discovery("agent note")),
            record("ent_b", discovery("newer")),
        ],
    );
    let dismiss = |id: &str| Proposal::from(WorkEdit::DismissEntry { entry_id: eid(id) });
    assert!(refuse(&state, main_agent(), vec![dismiss("ent_u")]).needs_user());
    submit(&mut state, main_agent(), vec![dismiss("ent_a")]);
    assert_eq!(entry_status(&state, "ent_a"), EntryStatus::Dismissed);
    let supersede = |by: &str| Proposal::SetEntryStatus {
        entry_id: eid("ent_b"),
        status: EntryStatus::Superseded { by: eid(by) },
    };
    assert!(
        refuse(&state, main_agent(), vec![supersede("ent_b")])
            .to_string()
            .contains("itself")
    );
    assert_eq!(
        refuse(&state, main_agent(), vec![supersede("ent_z")]).reasons,
        [Reason::UnknownEntry(eid("ent_z"))]
    );
    submit(&mut state, main_agent(), vec![supersede("ent_u")]);
    let back = Proposal::SetEntryStatus {
        entry_id: eid("ent_a"),
        status: EntryStatus::Proposed,
    };
    assert!(
        refuse(&state, user(), vec![back])
            .to_string()
            .contains("unconfirmed")
    );
}
