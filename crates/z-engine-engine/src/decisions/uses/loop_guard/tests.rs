//! Loop fixtures: a repeated call and a failure after an edit remind at
//! once (even with the model down); a stuck run reminds only on the model's
//! confident "no progress"; healthy progress never triggers. After the
//! turn's reminders the next allowed call asks (never in bypass mode), Ask
//! and Deny pass through, and off or shadow leave every result alone.

use serde_json::json;
use z_engine_config::{FeatureId, FeatureMode};
use z_engine_decisions::DecisionRecord;
use z_engine_policy::Decision;
use z_engine_protocol::{
    CallId, MessageId, PermissionMode, ToolResultPart, ToolStatus, TurnId, TurnOutcome, TurnRecord,
    VerificationOutcome, now_ms,
};
use z_engine_tools::names;

use super::detect::Finding;
use super::guard::{AFTER_EDIT, FAILURE, PROGRESS, REPEAT, reminder};
use super::steps::FailureClass;
use crate::batch::ToolCall;
use crate::decisions::seams::{annotate_result, review_call};
use crate::decisions::uses::scripted::{
    Scripted, install, main_run, notices, session, session_with, traced,
};
use crate::run::RunContext;

const FEATURE: FeatureId = FeatureId::DecisionsLoopGuard;
const ASSERTION: &str = "test parse ... FAILED\nassertion failed: left == right at src/lib.rs:12";
const BUILD: &str = "error: build script failed\nExit code 101";

fn call(name: &str, input: serde_json::Value) -> ToolCall {
    ToolCall {
        id: CallId::from("call"),
        name: name.into(),
        input,
    }
}

fn bash(command: &str) -> ToolCall {
    call(names::BASH, json!({ "command": command }))
}

fn grep(pattern: &str) -> ToolCall {
    call(names::GREP, json!({ "pattern": pattern }))
}

fn edit(path: &str) -> ToolCall {
    call(names::EDIT, json!({ "file_path": path }))
}

/// Runs the after-call seam on `call`'s result; returns the notes it added.
async fn finish(ctx: &RunContext, call: &ToolCall, status: ToolStatus, text: &str) -> Vec<String> {
    let mut content = vec![ToolResultPart::Text { text: text.into() }];
    annotate_result(ctx, call, status, &mut content).await;
    content.pop();
    let notes = content.into_iter().filter_map(|part| match part {
        ToolResultPart::Text { text } => Some(text),
        _ => None,
    });
    notes.collect()
}

fn allow() -> Decision {
    Decision::Allow {
        reason: "allow rule".into(),
    }
}

fn traced_now(ctx: &RunContext, wanted: impl Fn(&DecisionRecord) -> bool) -> bool {
    ctx.core.decisions.trace().recent(100).iter().any(wanted)
}

#[tokio::test]
async fn a_repeated_call_reminds_on_the_third_time_even_with_the_model_down() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, Scripted::down());
    for _ in 0..2 {
        assert!(
            finish(&ctx, &grep("TODO"), ToolStatus::Ok, "a.rs:1: TODO")
                .await
                .is_empty()
        );
    }
    let notes = finish(&ctx, &grep("TODO"), ToolStatus::Ok, "a.rs:1: TODO").await;
    assert_eq!(notes.len(), 1);
    assert!(
        notes[0].contains("Grep with the same input 3 times"),
        "{notes:?}"
    );
    assert_eq!(notices(&events).len(), 1);
    let rule = |record: &DecisionRecord| {
        record.question == REPEAT && record.outcome == "reminded (1 of 3)"
    };
    assert!(traced_now(&ctx, rule));
    assert!(
        finish(&ctx, &grep("TODO"), ToolStatus::Ok, "a.rs:1: TODO")
            .await
            .is_empty()
    );
    handle.close("test").await;
}

#[tokio::test]
async fn the_same_failure_after_an_edit_points_at_the_code() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, Scripted::default());
    let test = bash("cargo test");
    assert!(
        finish(&ctx, &test, ToolStatus::Error, ASSERTION)
            .await
            .is_empty()
    );
    assert!(
        finish(&ctx, &edit("src/lib.rs"), ToolStatus::Ok, "edited")
            .await
            .is_empty()
    );
    let shifted = ASSERTION.replace(":12", ":14");
    let notes = finish(&ctx, &test, ToolStatus::Error, &shifted).await;
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("the change did not fix it"), "{notes:?}");
    assert!(traced_now(&ctx, |record| record.question == AFTER_EDIT));
    handle.close("test").await;
}

#[tokio::test]
async fn healthy_progress_never_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    let model = Scripted::default().yes(PROGRESS, false);
    install(&handle, FEATURE, FeatureMode::On, model);
    let test = bash("cargo test");
    let steps = [
        (grep("parse"), ToolStatus::Ok, "src/lib.rs:3: fn parse"),
        (grep("parse"), ToolStatus::Ok, "src/lib.rs:3: fn parse"),
        (
            call(names::READ, json!({ "file_path": "src/lib.rs" })),
            ToolStatus::Ok,
            "fn parse",
        ),
        (test.clone(), ToolStatus::Error, ASSERTION),
        (edit("src/lib.rs"), ToolStatus::Ok, "edited"),
        (
            test.clone(),
            ToolStatus::Error,
            "error[E0308]: mismatched types",
        ),
        (edit("src/lib.rs"), ToolStatus::Ok, "edited"),
        (test.clone(), ToolStatus::Ok, "test result: ok. 4 passed"),
        (call(names::TODO_WRITE, json!({})), ToolStatus::Ok, "ok"),
        (call(names::TODO_WRITE, json!({})), ToolStatus::Ok, "ok"),
        (call(names::TODO_WRITE, json!({})), ToolStatus::Ok, "ok"),
    ];
    for (call, status, text) in steps {
        assert!(
            finish(&ctx, &call, status, text).await.is_empty(),
            "{}",
            call.name
        );
    }
    assert!(notices(&events).is_empty());
    assert!(ctx.core.decisions.trace().recent(10).is_empty());
    handle.close("test").await;
}

/// The same failure from three different builds, in a turn of its own;
/// returns the notes on the last one.
async fn stuck_run(ctx: &RunContext) -> Vec<String> {
    let mut notes = Vec::new();
    for target in ["a", "b", "c"] {
        let call = bash(&format!("cargo build -p {target}"));
        notes = finish(ctx, &call, ToolStatus::Error, BUILD).await;
    }
    ctx.core
        .with_state(|state| state.turns.push(finished_turn()));
    notes
}

async fn repeat_grep(ctx: &RunContext) {
    for _ in 0..3 {
        finish(ctx, &grep("TODO"), ToolStatus::Ok, "none").await;
    }
}

#[tokio::test]
async fn a_stuck_run_reminds_only_on_a_confident_no_progress() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let ctx = main_run(&handle);
    let unsure = [
        Scripted::down(),
        Scripted::default(),
        Scripted::default().yes(PROGRESS, true),
    ];
    for model in unsure {
        install(&handle, FEATURE, FeatureMode::On, model);
        assert!(stuck_run(&ctx).await.is_empty());
    }
    let no_progress = Scripted::default()
        .yes(PROGRESS, false)
        .choice(FAILURE, "environment");
    install(&handle, FEATURE, FeatureMode::On, no_progress);
    let notes = stuck_run(&ctx).await;
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("points at the environment"), "{notes:?}");
    handle.close("test").await;
}

#[tokio::test]
async fn after_its_reminders_the_next_allowed_call_asks_unless_bypassing() {
    let dir = tempfile::tempdir().unwrap();
    let one = "[decisions.loop_guard]\nmax_reminders = 1\n";
    let (handle, events) = session_with(dir.path(), Some(one)).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, Scripted::down());
    repeat_grep(&ctx).await;
    assert_eq!(review_call(&ctx, &grep("x"), allow()).await, allow());
    repeat_grep(&ctx).await;
    assert!(
        notices(&events)
            .last()
            .unwrap()
            .contains("next call asks you first")
    );
    let deny = Decision::Deny {
        reason: "policy".into(),
    };
    assert_eq!(review_call(&ctx, &grep("x"), deny.clone()).await, deny);
    let asked = review_call(&ctx, &grep("x"), allow()).await;
    assert!(
        matches!(&asked, Decision::Ask { reason, .. } if reason.starts_with("Loop guard")),
        "{asked:?}"
    );
    assert_eq!(
        review_call(&ctx, &grep("x"), allow()).await,
        allow(),
        "asks once"
    );

    handle
        .core
        .with_state(|state| state.mode = PermissionMode::Bypass);
    repeat_grep(&ctx).await;
    repeat_grep(&ctx).await;
    assert!(
        notices(&events)
            .last()
            .unwrap()
            .contains("Bypass mode does not ask")
    );
    assert_eq!(review_call(&ctx, &grep("x"), allow()).await, allow());
    handle.close("test").await;
}

#[tokio::test]
async fn off_and_shadow_leave_results_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    for mode in [FeatureMode::Off, FeatureMode::Shadow] {
        install(&handle, FEATURE, mode, Scripted::down());
        for _ in 0..3 {
            let notes = finish(&ctx, &grep("TODO"), ToolStatus::Ok, "none").await;
            assert!(notes.is_empty(), "{mode:?}");
        }
    }
    let would = |record: &DecisionRecord| record.shadow && record.outcome == "reminded (1 of 3)";
    assert!(
        traced(&handle, would).await,
        "shadow records the reminder it would give"
    );
    assert!(notices(&events).is_empty());
    handle.close("test").await;
}

#[test]
fn reminders_fit_the_failure() {
    let after_edit = |class| Finding::FailedAfterEdit {
        tool: "Bash".into(),
        count: 2,
        failure: 1,
        class,
    };
    let retry = reminder(
        &after_edit(Some(FailureClass::Retry)),
        Some(FailureClass::Retry),
    );
    assert!(retry.contains("looks transient"));
    assert!(reminder(&after_edit(None), None).contains("did not fix it"));
    let suspect = Finding::Suspect {
        tool: "Bash".into(),
        count: 4,
        same_failure: false,
        class: None,
    };
    assert!(reminder(&suspect, None).contains("last 4 steps"));
}

fn finished_turn() -> TurnRecord {
    let now = now_ms();
    TurnRecord {
        turn_id: TurnId::new(),
        message_id: MessageId::new(),
        outcome: TurnOutcome::Completed,
        verification: VerificationOutcome::NotApplicable,
        usage: Default::default(),
        cost_usd: 0.0,
        started_at: now,
        finished_at: now,
    }
}
