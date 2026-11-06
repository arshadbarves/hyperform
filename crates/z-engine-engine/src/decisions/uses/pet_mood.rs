//! Pet mood (`decisions_pet_mood`): when a turn ends, how it went (smooth,
//! struggling, blocked, done well), from the request, the outcome, the
//! verification badge, the calls that ran and the agent's last sentence.
//! The answer reaches the GUI as `TurnToneJudged`; the pet's mood uses it
//! as one more input and keeps its event-based mood without one. Turns the
//! user stopped are never judged.

use async_trait::async_trait;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_decisions::{DecisionRequest, Question};
use z_engine_prompts::decisions::PET_MOOD_TONE;
use z_engine_protocol::decisions::TurnTone;
use z_engine_protocol::{TurnOutcome, TurnRecord, VerificationOutcome};

use super::digest::{head, last_line, latest_assistant_text, latest_request, turn_calls};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

pub(super) const QUESTION: &str = "pet_mood_tone";
const REQUEST_CHARS: usize = 600;
const SENTENCE_CHARS: usize = 300;
const CALLS_SHOWN: usize = 8;

#[derive(Debug)]
pub(crate) struct PetMood;

pub(crate) static PET_MOOD: PetMood = PetMood;

#[async_trait]
impl DecisionUse for PetMood {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsPetMood
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::TurnEnd]
    }

    async fn turn_end(&self, cx: &UseContext, turn: &TurnRecord) -> Option<TurnTone> {
        let ended = outcome_label(&turn.outcome)?;
        let question = Question::choice(QUESTION, PET_MOOD_TONE).ok()?;
        let request = DecisionRequest::new(state(cx, ended, &turn.verification)).ask(question);
        let answer = cx.ask(&request).await.into_iter().next()?;
        let tone = answer.choice().and_then(tone_of);
        let outcome = match (tone, cx.shadow) {
            (None, _) => "unchanged".to_string(),
            (Some(_), true) => format!("would show {}", answer.choice().unwrap_or_default()),
            (Some(_), false) => format!("showed {}", answer.choice().unwrap_or_default()),
        };
        cx.record(
            cx.record_of(&answer, &request.fingerprint())
                .outcome(&outcome),
        );
        tone
    }
}

/// `None` for turns the user stopped or the app interrupted.
fn outcome_label(outcome: &TurnOutcome) -> Option<&'static str> {
    match outcome {
        TurnOutcome::Completed => Some("completed"),
        TurnOutcome::Failed { .. } => Some("failed with an error"),
        TurnOutcome::BudgetExhausted { .. } => Some("stopped by a turn or cost budget"),
        TurnOutcome::Cancelled | TurnOutcome::Interrupted => None,
    }
}

fn verification_label(verification: &VerificationOutcome) -> &'static str {
    match verification {
        VerificationOutcome::NotApplicable => "nothing changed",
        VerificationOutcome::Unverified { .. } => "changes not verified",
        VerificationOutcome::Verified { .. } => "checks passed",
        VerificationOutcome::Failed { .. } => "checks failed",
    }
}

fn tone_of(key: &str) -> Option<TurnTone> {
    match key {
        "smooth" => Some(TurnTone::Smooth),
        "struggling" => Some(TurnTone::Struggling),
        "blocked" => Some(TurnTone::Blocked),
        "done_well" => Some(TurnTone::DoneWell),
        _ => None,
    }
}

fn state(cx: &UseContext, ended: &str, verification: &VerificationOutcome) -> serde_json::Value {
    let (request, words, calls) = cx.core.with_state(|state| {
        let working = &state.working;
        let calls = turn_calls(working, usize::MAX);
        (
            latest_request(working),
            latest_assistant_text(working),
            calls,
        )
    });
    let failed = calls
        .iter()
        .filter(|call| call.ends_with(" (error)"))
        .count();
    let shown = &calls[calls.len().saturating_sub(CALLS_SHOWN)..];
    json!({
        "request": head(&request, REQUEST_CHARS),
        "ended": ended,
        "verification": verification_label(verification),
        "tool_calls": calls.len(),
        "failed_calls": failed,
        "latest_calls": shown,
        "last_sentence": last_line(&words, SENTENCE_CHARS),
    })
}

#[cfg(test)]
#[path = "pet_mood_tests.rs"]
mod tests;
