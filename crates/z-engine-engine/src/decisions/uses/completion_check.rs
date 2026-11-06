//! Completion check (`decisions_completion_check`): whether the agent's
//! final message claims success while no check backs the turn. Four
//! questions about the final message and what ran this turn; a claim needs
//! confident answers that verification applies and that the message says
//! the work is complete or that checks passed. The verifier decides what a
//! claim does; it can never mark a turn Verified.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_config::FeatureId;
use z_engine_decisions::{Answer, DecisionRequest, Question, UNCHANGED};
use z_engine_prompts::decisions::{
    COMPLETION_CLAIMS_CHECKS, COMPLETION_CLAIMS_DONE, COMPLETION_STATE,
    COMPLETION_VERIFICATION_APPLIES,
};
use z_engine_protocol::decisions::UncheckedClaim;

use super::digest::{head, latest_assistant_text, latest_request, tail, turn_calls};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

const DONE: &str = "completion_claims_done";
const CHECKS: &str = "completion_claims_checks";
const APPLIES: &str = "completion_verification_applies";
const STATE: &str = "completion_state";
const MESSAGE_CHARS: usize = 1_500;
const REQUEST_CHARS: usize = 400;
const CALLS: usize = 20;
const FILES: usize = 20;

#[derive(Debug)]
pub(crate) struct CompletionCheck;

pub(crate) static COMPLETION_CHECK: CompletionCheck = CompletionCheck;

#[async_trait]
impl DecisionUse for CompletionCheck {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsCompletionCheck
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::Completion]
    }

    async fn completion(&self, cx: &UseContext, changed: &[PathBuf]) -> Option<UncheckedClaim> {
        let questions = [
            Question::yes_no(DONE, COMPLETION_CLAIMS_DONE),
            Question::yes_no(CHECKS, COMPLETION_CLAIMS_CHECKS),
            Question::yes_no(APPLIES, COMPLETION_VERIFICATION_APPLIES),
            Question::choice(STATE, COMPLETION_STATE),
        ];
        let mut request = DecisionRequest::new(state(cx, changed));
        for question in questions {
            request = request.ask(question.ok()?);
        }
        let answers = cx.ask(&request).await;
        let claim = judge(&answers);
        let outcome = if claim.is_some() {
            "claimed"
        } else {
            UNCHANGED
        };
        let fingerprint = request.fingerprint();
        for answer in &answers {
            cx.record(cx.record_of(answer, &fingerprint).outcome(outcome));
        }
        claim
    }
}

/// A claim only from confident answers: verification applies, and the
/// message says checks passed or that the work is done and complete.
fn judge(answers: &[Answer]) -> Option<UncheckedClaim> {
    let find = |name: &str| answers.iter().find(|answer| answer.question == name);
    let yes = |name: &str| find(name).and_then(Answer::yes) == Some(true);
    if !yes(APPLIES) {
        return None;
    }
    let checks = yes(CHECKS);
    let done = yes(DONE) && find(STATE).and_then(Answer::choice) == Some("complete");
    (done || checks).then_some(UncheckedClaim { done, checks })
}

fn state(cx: &UseContext, changed: &[PathBuf]) -> Value {
    let (request, message, calls) = cx.core.with_state(|state| {
        let working = &state.working;
        let calls = turn_calls(working, CALLS);
        (
            latest_request(working),
            latest_assistant_text(working),
            calls,
        )
    });
    let root = &cx.core.root;
    let files: Vec<String> = changed
        .iter()
        .take(FILES)
        .map(|path| {
            path.strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string()
        })
        .collect();
    json!({
        "request": head(&request, REQUEST_CHARS),
        "final_message": tail(&message, MESSAGE_CHARS),
        "tool_calls": calls,
        "changed_files": files,
    })
}

#[cfg(test)]
#[path = "completion_check_tests.rs"]
mod tests;
