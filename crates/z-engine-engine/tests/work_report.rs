//! A helper's report as the kernel folds it: the files come from the
//! worktree diff and the checks from the check records of its run, while
//! the helper's own lists are kept apart as claims (in the shared tree,
//! with no diff to read, the files stay unknown). A helper that ends
//! without a report gets one the kernel writes from what it observed, and
//! its task is blocked as unreported. A report saved earlier in the run is
//! still recorded when the helper then completes its task without one.

mod support;

use serde_json::json;
use support::{
    Harness, last_user_text, new_task, parallel_settings, report, route_task, update_task,
    work_settings,
};
use z_engine_protocol::work::{
    BlockReason, EvidenceRef, Integration, ReportConfidence, ReportSource, TaskReport, TaskStatus,
};
use z_engine_testkit::{FixtureRepo, Script};

const ASK: &str = "[permissions]\nmode = \"bypass\"\n\n[work]\nmerge = \"ask\"\n\n\
    [[verification.checks]]\nid = \"unit\"\nlabel = \"Unit tests\"\ncommand = \"echo all good\"\nkind = \"test\"\n";

/// One task over `src`; with `parallel` its helper gets a worktree.
async fn planned(parallel: bool) -> Harness {
    let settings = if parallel {
        parallel_settings(ASK)
    } else {
        work_settings(ASK)
    };
    let repo = FixtureRepo::git(&[("src/lib.rs", "lib\n")]);
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    let scope = json!({ "scope": ["src"] });
    h.model
        .push(Script::tool("TaskCreate", new_task("Add real", scope)));
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan it").await;
    if parallel {
        h.approve_parallel().await;
    }
    h
}

/// A helper that writes `src/real.rs`, runs the unit check and claims
/// other files and tests than it changed and ran.
fn claiming_helper(h: &Harness, marker: &'static str) {
    let claims = json!({
        "summary": "Added real",
        "filesChanged": ["src/claimed.rs"],
        "testsRun": ["cargo test --all"],
        "confidence": "high"
    });
    let rounds = vec![
        Script::tools(&[
            (
                "Write",
                json!({ "file_path": "src/real.rs", "content": "real\n" }),
            ),
            ("Verify", json!({ "action": "run", "check": "custom:unit" })),
        ]),
        Script::tool("TaskUpdate", report("t1", claims)),
        Script::text("Reported."),
    ];
    route_task(&h.model, marker, rounds);
}

async fn reported(h: &mut Harness) -> (TaskStatus, Option<Integration>, TaskReport) {
    let task = h.task_until("t1", |t| t.run.report.is_some()).await;
    (task.status, task.run.integration, task.run.report.unwrap())
}

fn assert_claims_kept(report: &TaskReport) {
    assert_eq!(report.source, ReportSource::Worker);
    assert_eq!(report.summary, "Added real");
    assert_eq!(report.confidence, Some(ReportConfidence::High));
    assert_eq!(report.claimed.files_changed, ["src/claimed.rs"]);
    assert_eq!(report.claimed.tests_run, ["cargo test --all"]);
    assert!(
        matches!(
            report.tests_run.as_slice(),
            [EvidenceRef::Check { passed: true, .. }]
        ),
        "{:?}",
        report.tests_run
    );
}

#[tokio::test]
async fn observed_files_and_checks_replace_the_helpers_claims() {
    let mut h = planned(true).await;
    claiming_helper(&h, "REPORT-CLAIMS");
    let replies = h.dispatch_turn(&[("t1", "REPORT-CLAIMS")]).await;
    assert!(!replies[0].0, "{replies:?}");

    let (_, _, report) = reported(&mut h).await;
    assert_claims_kept(&report);
    assert!(report.diff_known);
    assert_eq!(report.files_changed, ["src/real.rs"]);
    let waiting = h
        .task_until("t1", |t| {
            matches!(t.run.integration, Some(Integration::PendingApply { .. }))
        })
        .await;
    assert_eq!(waiting.status, TaskStatus::Active);
}

#[tokio::test]
async fn in_the_shared_tree_claimed_files_stay_claims() {
    let mut h = planned(false).await;
    claiming_helper(&h, "REPORT-SHARED");
    let replies = h.dispatch_turn(&[("t1", "REPORT-SHARED")]).await;
    assert!(!replies[0].0, "{replies:?}");

    let (_, _, report) = reported(&mut h).await;
    assert_claims_kept(&report);
    assert!(!report.diff_known);
    assert!(
        report.files_changed.is_empty(),
        "{:?}",
        report.files_changed
    );
    let applied = h.task_until("t1", |t| t.run.integration.is_some()).await;
    let files = vec!["src/real.rs".to_string()];
    assert_eq!(
        applied.run.integration,
        Some(Integration::Applied { files })
    );
}

#[tokio::test]
async fn a_helper_without_a_report_leaves_the_task_unreported_and_blocked() {
    let mut h = planned(true).await;
    let write = json!({ "file_path": "src/quiet.rs", "content": "quiet\n" });
    let rounds = vec![
        Script::tool("Write", write),
        Script::text("All done, trust me."),
    ];
    route_task(&h.model, "REPORT-NONE", rounds);
    let replies = h.dispatch_turn(&[("t1", "REPORT-NONE")]).await;
    assert!(!replies[0].0, "{replies:?}");

    let (status, _, report) = reported(&mut h).await;
    assert_eq!(
        status,
        TaskStatus::Blocked {
            reason: BlockReason::Unreported
        }
    );
    assert_eq!(report.source, ReportSource::Unreported);
    assert_eq!(report.files_changed, ["src/quiet.rs"]);
    assert!(report.claimed.files_changed.is_empty());
    let told = last_user_text(h.main_requests().last().unwrap());
    assert!(told.contains("ended without a report"), "{told}");
}

#[tokio::test]
async fn a_saved_report_is_kept_when_the_helper_then_completes_its_task() {
    let mut h = planned(false).await;
    let saved = json!({ "summary": "Added real early", "confidence": "medium" });
    let rounds = vec![
        Script::tool("TaskUpdate", report("t1", saved)),
        Script::tools(&[
            (
                "Write",
                json!({ "file_path": "src/real.rs", "content": "real\n" }),
            ),
            ("Verify", json!({ "action": "run", "check": "custom:unit" })),
        ]),
        Script::tool(
            "TaskUpdate",
            update_task("t1", json!({ "status": "completed" })),
        ),
        Script::text("Done."),
    ];
    route_task(&h.model, "REPORT-THEN-DONE", rounds);
    let replies = h.dispatch_turn(&[("t1", "REPORT-THEN-DONE")]).await;
    assert!(!replies[0].0, "{replies:?}");

    let done = h.finished("t1").await;
    assert_eq!(done.status, TaskStatus::Done, "{done:?}");
    let report = done.run.report.expect("the saved report is recorded");
    assert_eq!(report.source, ReportSource::Worker);
    assert_eq!(report.summary, "Added real early");
    assert_eq!(report.confidence, Some(ReportConfidence::Medium));
}
