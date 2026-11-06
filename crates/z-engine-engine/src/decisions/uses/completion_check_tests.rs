//! A claim needs confident answers; it reaches the receipt only in `on`
//! mode and never changes the badge the verifier computed.

use std::collections::BTreeMap;

use z_engine_config::{FeatureId, FeatureMode};
use z_engine_decisions::{AbstainReason, Answer, DecisionRecord, Verdict};
use z_engine_protocol::decisions::UncheckedClaim;
use z_engine_protocol::{ContentBlock, Event, Message, Role, VerificationOutcome};

use super::{APPLIES, CHECKS, DONE, STATE, judge};
use crate::decisions::uses::scripted::{Events, Scripted, install, main_run, session_with, traced};
use crate::session::SessionHandle;
use crate::verify::{ModeVerifier, StopVerdict, Verifier};

const FEATURE: FeatureId = FeatureId::DecisionsCompletionCheck;

fn answer(question: &str, verdict: Verdict) -> Answer {
    Answer {
        question: question.into(),
        confidence: Some(0.95),
        proposal: Some(verdict),
        probabilities: BTreeMap::new(),
        abstain: None,
        provider: "test".into(),
        latency_ms: 1,
        cached: false,
    }
}

fn answers(done: bool, checks: bool, applies: bool, state: &str) -> Vec<Answer> {
    vec![
        answer(DONE, Verdict::YesNo(done)),
        answer(CHECKS, Verdict::YesNo(checks)),
        answer(APPLIES, Verdict::YesNo(applies)),
        answer(STATE, Verdict::Choice(state.into())),
    ]
}

fn claiming() -> Scripted {
    Scripted::default()
        .yes(DONE, true)
        .yes(CHECKS, true)
        .yes(APPLIES, true)
        .choice(STATE, "complete")
}

#[test]
fn only_confident_answers_make_a_claim() {
    let both = UncheckedClaim {
        done: true,
        checks: true,
    };
    assert_eq!(judge(&answers(true, true, true, "complete")), Some(both));
    let checks = UncheckedClaim {
        done: false,
        checks: true,
    };
    assert_eq!(judge(&answers(true, true, true, "partial")), Some(checks));
    assert_eq!(judge(&answers(true, false, true, "blocked")), None);
    assert_eq!(judge(&answers(true, true, false, "complete")), None);
    let mut unsure = answers(true, true, true, "complete");
    unsure[2] = Answer::abstained(APPLIES, AbstainReason::LowConfidence, "test");
    assert_eq!(judge(&unsure), None);
    assert_eq!(judge(&[]), None);
}

/// A report-mode (or untrusted auto-mode) session whose turn changed a file
/// and ended with "all tests pass".
async fn claimed_turn(dir: &std::path::Path, mode: &str) -> (SessionHandle, Events) {
    let settings = format!("schema = 2\n\n[verification]\nmode = \"{mode}\"\n");
    let (handle, events) = session_with(dir, Some(&settings)).await;
    handle.core.with_state(|state| {
        state.mutation.touch(1);
        let ask = Message::new(Role::User, vec![ContentBlock::text("fix the parser")]);
        let done = "Fixed the parser; all tests pass.";
        let reply = Message::new(Role::Assistant, vec![ContentBlock::text(done)]);
        state.working.extend([ask, reply]);
    });
    (handle, events)
}

fn claims(events: &Events) -> Vec<UncheckedClaim> {
    let events = events.lock().unwrap();
    let claims = events.iter().filter_map(|event| match event {
        Event::CompletionClaimUnchecked { claim } => Some(*claim),
        _ => None,
    });
    claims.collect()
}

async fn stop(handle: &SessionHandle) -> StopVerdict {
    let changed = [handle.core.root.join("src/parser.rs")];
    ModeVerifier.at_stop(&main_run(handle), 0, &changed).await
}

fn unverified(verdict: &StopVerdict) -> bool {
    matches!(
        verdict,
        StopVerdict::Done(VerificationOutcome::Unverified { .. })
    )
}

#[tokio::test]
async fn report_mode_puts_a_confident_claim_on_the_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = claimed_turn(dir.path(), "report").await;
    install(&handle, FEATURE, FeatureMode::On, claiming());
    let verdict = stop(&handle).await;
    assert!(unverified(&verdict), "{verdict:?}");
    assert_eq!(
        claims(&events),
        [UncheckedClaim {
            done: true,
            checks: true
        }]
    );
    handle.close("test").await;
}

#[tokio::test]
async fn off_down_unsure_and_shadow_leave_the_receipt_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = claimed_turn(dir.path(), "report").await;
    let baseline = stop(&handle).await;
    for (mode, model) in [
        (FeatureMode::Off, claiming()),
        (FeatureMode::On, Scripted::down()),
        (FeatureMode::On, Scripted::default()),
        (FeatureMode::Shadow, claiming()),
    ] {
        install(&handle, FEATURE, mode, model);
        assert_eq!(stop(&handle).await, baseline, "{mode:?}");
    }
    let would_claim = |record: &DecisionRecord| record.shadow && record.outcome == "claimed";
    assert!(traced(&handle, would_claim).await);
    assert!(claims(&events).is_empty());
    handle.close("test").await;
}

#[tokio::test]
async fn auto_mode_flags_a_claim_when_no_check_can_run() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = claimed_turn(dir.path(), "auto").await;
    let baseline = stop(&handle).await;
    install(&handle, FEATURE, FeatureMode::On, claiming());
    assert_eq!(stop(&handle).await, baseline, "the badge stays as it was");
    assert!(unverified(&baseline));
    assert_eq!(claims(&events).len(), 1, "untrusted projects run no checks");
    handle.close("test").await;
}

#[tokio::test]
async fn turns_without_changes_are_not_asked_about() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = claimed_turn(dir.path(), "report").await;
    handle
        .core
        .with_state(|state| state.mutation = Default::default());
    install(&handle, FEATURE, FeatureMode::On, claiming());
    let verdict = stop(&handle).await;
    assert_eq!(
        verdict,
        StopVerdict::Done(VerificationOutcome::NotApplicable)
    );
    assert!(claims(&events).is_empty());
    assert!(handle.core.decisions.trace().recent(10).is_empty());
    handle.close("test").await;
}
