//! Every decision feature on against a decision model that answers every
//! question confidently, always with the answer that acts (escalate, keep,
//! suggest, route). The scripted session (two tasks, approvals, a denied
//! command, a stop with changes, a compaction, a question) still completes
//! with no lost events, and the safety rules hold: every call the policy
//! asks about still asks, a denied call never runs or asks, no turn is
//! cancelled, and no turn is marked Verified.

mod laya;
mod support;

use std::collections::BTreeSet;

use laya::workout::{self, BASE};
use serde_json::Value;
use z_engine_protocol::{Event, TurnOutcome, VerificationOutcome};

/// The acting answer to each choice question; yes to every yes/no.
fn acting(name: &str, _state: &Value) -> &'static str {
    match name {
        "risk_intent" => "off_task",
        "risk_level" => "harmful",
        "routing_complexity" => "complex",
        "plan_suggest_size" => "large",
        "loop_guard_failure" => "code",
        "pet_mood_tone" => "struggling",
        "completion_state" => "complete",
        "inbox_urgency" => "high",
        "task_boundary" => "unrelated",
        _ => laya::YES,
    }
}

/// A user rule asking before every Bash command, and one reminding.
const RULES: &str = "\n[[decisions.rules]]\nevent = \"PreToolUse\"\nmatcher = \"Bash\"\n\
                     question = \"Does this command touch production?\"\naction = \"ask\"\n\n\
                     [[decisions.rules]]\nevent = \"UserPromptSubmit\"\n\
                     question = \"Does this message set a deadline?\"\naction = \"remind\"\n";

#[tokio::test]
async fn a_confident_model_never_loosens_or_loses_anything() {
    let mut off = workout::run(BASE, false).await;
    let endpoint = laya::laya(acting).await;
    let settings = laya::every_feature(BASE, &endpoint, "on", laya::ANSWER_TIMEOUT_MS) + RULES;
    let mut on = workout::run(&settings, false).await;

    assert_eq!(on.turns.len(), 3);
    for turn in &on.turns {
        assert_eq!(turn.outcome, TurnOutcome::Completed, "{turn:?}");
        let verified = matches!(turn.verification, VerificationOutcome::Verified { .. });
        assert!(!verified, "{turn:?}");
    }

    let (off_asked, on_asked) = (off.asked(), on.asked());
    for card in &off_asked {
        assert!(on_asked.contains(card), "{card:?} still asks: {on_asked:?}");
    }
    assert!(
        on_asked.len() > off_asked.len(),
        "allowed calls were escalated: {on_asked:?}"
    );
    let denied = |statuses: &[(String, String)]| {
        let denied = statuses.iter().filter(|(_, status)| status == "Denied");
        denied.count()
    };
    let (off_statuses, on_statuses) = (off.statuses(), on.statuses());
    assert_eq!(denied(&off_statuses), 1);
    assert_eq!(denied(&on_statuses), 1, "{on_statuses:?}");
    assert!(on_asked.iter().all(|(_, title)| !title.contains("rm ")));

    let seen = on.h.events.seen().to_vec();
    let count = |wanted: fn(&Event) -> bool| seen.iter().filter(|e| wanted(e)).count();
    assert_eq!(count(|e| matches!(e, Event::TurnStarted { .. })), 3);
    assert_eq!(count(|e| matches!(e, Event::TurnFinished { .. })), 3);
    let ids = |pick: fn(&Event) -> Option<String>| -> BTreeSet<String> {
        seen.iter().filter_map(pick).collect()
    };
    let started = ids(|e| match e {
        Event::ToolStarted { call_id, .. } => Some(call_id.0.clone()),
        _ => None,
    });
    let finished = ids(|e| match e {
        Event::ToolFinished { call_id, .. } => Some(call_id.0.clone()),
        _ => None,
    });
    assert_eq!(started, finished, "every tool call finished");
    let requested = ids(|e| match e {
        Event::ApprovalRequested { request } => Some(request.request_id.0.clone()),
        _ => None,
    });
    let resolved = ids(|e| match e {
        Event::ApprovalResolved { request_id, .. } => Some(request_id.0.clone()),
        _ => None,
    });
    assert_eq!(requested, resolved, "every approval was resolved");

    on.h.expect(|e| matches!(e, Event::RouteChosen { .. }))
        .await;
    on.h.expect(|e| matches!(e, Event::CompletionClaimUnchecked { .. }))
        .await;
    on.h.expect(|e| matches!(e, Event::Suggested { .. })).await;
    on.h.expect(|e| matches!(e, Event::TurnToneJudged { .. }))
        .await;
    let decisions = on.h.engine.session_decisions(&on.h.session, 1000).unwrap();
    assert!(decisions.records.iter().all(|record| !record.shadow));
    let features: BTreeSet<&str> = decisions
        .records
        .iter()
        .map(|r| r.feature.as_str())
        .collect();
    assert!(features.len() >= 10, "{features:?}");
    let asked_user = |w: &workout::Workout| {
        let asked = w.h.events.seen().iter();
        asked
            .filter(|e| matches!(e, Event::QuestionAsked { .. }))
            .count()
    };
    assert_eq!(asked_user(&off), 1);
    assert_eq!(asked_user(&on), 0, "pointed at the earlier answer instead");
}
