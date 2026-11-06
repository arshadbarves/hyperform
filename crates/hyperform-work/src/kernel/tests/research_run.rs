use hyperform_protocol::work::{
    Actor, AuditSource, AuditStatus, Integration, ParallelPolicy, PlanStatus, Provenance,
    ReportDraft, TaskStatus,
};

use super::run::{bound, running, send};
use crate::testkit::*;
use crate::{Facts, Proposal, Reason};

fn tainted(facts: Facts) -> Facts {
    Facts {
        untrusted_input: true,
        ..facts
    }
}

fn report(discovery: &str) -> Proposal {
    Proposal::Report {
        task_id: tid("a"),
        report: Some(Box::new(ReportDraft {
            summary: "Read the release notes".into(),
            discoveries: vec![discovery.into()],
            decisions: vec!["use a client token".into()],
            ..ReportDraft::default()
        })),
    }
}

#[test]
fn a_tainted_turn_dispatches_accepted_tasks_but_not_its_own_proposals() {
    let mut state = plan(PlanStatus::Active);
    scoped(&mut state, "a", &[], &["src/a/**"]);
    submit(
        &mut state,
        user(),
        vec![Proposal::SetParallel {
            parallel: ParallelPolicy::Approved,
        }],
    );
    let facts = tainted(parallel_facts(3));
    submit_with(
        &mut state,
        main_agent(),
        vec![create_task(task("web"))],
        &facts,
    );
    assert_eq!(status_of(&state, "web"), TaskStatus::Proposed);
    let rejection = refuse_with(
        &state,
        main_agent(),
        vec![send("web", "w2", Some("/wt/web"))],
        &facts,
    );
    assert!(rejection.to_string().contains("proposed"), "{rejection}");

    submit_with(
        &mut state,
        main_agent(),
        vec![send("a", "w1", None)],
        &facts,
    );
    assert_eq!(status_of(&state, "a"), TaskStatus::Active);
}

#[test]
fn a_tainted_helper_reports_but_its_findings_stay_unconfirmed() {
    let mut state = running();
    submit_with(
        &mut state,
        worker("w1"),
        vec![report("the API needs a token")],
        &tainted(bound("a")),
    );
    let a = state.task(&tid("a")).unwrap();
    let folded = a.run.report.as_ref().unwrap();
    assert_eq!(folded.provenance, Provenance::Research);
    assert_eq!(folded.discoveries, ["the API needs a token"]);
    assert_eq!(a.result.as_deref(), Some("Read the release notes"));

    let mut clean = running();
    submit_with(&mut clean, worker("w1"), vec![report("x")], &bound("a"));
    let folded = clean.task(&tid("a")).unwrap().run.report.clone().unwrap();
    assert_eq!(folded.provenance, Provenance::Agent);
}

#[test]
fn a_tainted_turn_retries_alone_only_where_it_already_could() {
    let mut state = running();
    submit_with(
        &mut state,
        Actor::Kernel,
        vec![Proposal::Report {
            task_id: tid("a"),
            report: None,
        }],
        &parallel_facts(3),
    );
    let retry = || Proposal::RetrySerial { task_id: tid("a") };
    let rejection = refuse_with(&state, worker("w1"), vec![retry()], &tainted(bound("a")));
    assert!(matches!(rejection.reasons[0], Reason::WorkerScope { .. }));
    submit_with(
        &mut state,
        main_agent(),
        vec![retry()],
        &tainted(parallel_facts(3)),
    );
    assert_eq!(status_of(&state, "a"), TaskStatus::Pending);
    assert!(state.task(&tid("a")).unwrap().run.serial);
}

#[test]
fn integration_audits_and_findings_stay_with_the_kernel_and_the_user() {
    let state = running();
    let facts = tainted(parallel_facts(3));
    let integrate = Proposal::Integrate {
        task_id: tid("a"),
        integration: Integration::Discarded,
    };
    let audit = Proposal::Audit {
        source: AuditSource::Deterministic,
        findings: vec![],
    };
    for engine_only in [integrate, audit] {
        let rejection = refuse_with(&state, main_agent(), vec![engine_only], &facts);
        assert!(matches!(rejection.reasons[0], Reason::EngineOnly(_)));
    }
    let finding = Proposal::SetFindingStatus {
        finding_id: "f1".into(),
        status: AuditStatus::Dismissed,
    };
    let rejection = refuse_with(&state, main_agent(), vec![finding], &facts);
    assert!(matches!(rejection.reasons[0], Reason::UserOnly(_)));
}
