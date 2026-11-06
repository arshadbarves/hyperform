//! Inbox priority (`decisions_inbox_priority`): how urgent each approval,
//! question, plan review and warning or error notice is (low, normal,
//! high), next to the user's latest request. Scores reach the GUI as
//! `UrgencyScored` and sort the inbox within each group; `Notification`
//! hooks fire only when something is high. Nothing is answered, hidden or
//! allowed: an approval still waits for the user.

use async_trait::async_trait;
use futures::future::join_all;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_decisions::{DecisionRequest, Question};
use z_engine_prompts::decisions::INBOX_URGENCY;
use z_engine_protocol::decisions::Urgency;

use super::digest::{head, latest_request};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::decisions::seams::AttentionItem;

pub(super) const QUESTION: &str = "inbox_urgency";
const REQUEST_CHARS: usize = 400;
const TEXT_CHARS: usize = 600;

#[derive(Debug)]
pub(crate) struct InboxPriority;

pub(crate) static INBOX_PRIORITY: InboxPriority = InboxPriority;

#[async_trait]
impl DecisionUse for InboxPriority {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsInboxPriority
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::Attention]
    }

    async fn attention(&self, cx: &UseContext, items: &[AttentionItem]) -> Vec<Option<Urgency>> {
        let Ok(question) = Question::choice(QUESTION, INBOX_URGENCY) else {
            return vec![None; items.len()];
        };
        let task = cx.core.with_state(|state| latest_request(&state.working));
        let task = head(&task, REQUEST_CHARS);
        let scores = items.iter().map(|item| score(cx, &question, &task, item));
        join_all(scores).await
    }
}

async fn score(
    cx: &UseContext,
    question: &Question,
    task: &str,
    item: &AttentionItem,
) -> Option<Urgency> {
    let state = json!({
        "kind": item.kind.label(),
        "text": head(&item.text, TEXT_CHARS),
        "request": task,
    });
    let request = DecisionRequest::new(state).ask(question.clone());
    let answer = cx.ask(&request).await.into_iter().next()?;
    let urgency = answer.choice().and_then(urgency_of);
    let outcome = match (urgency, cx.shadow) {
        (None, _) => "unchanged".to_string(),
        (Some(_), true) => format!("would rate {}", answer.choice().unwrap_or_default()),
        (Some(_), false) => format!("rated {}", answer.choice().unwrap_or_default()),
    };
    cx.record(
        cx.record_of(&answer, &request.fingerprint())
            .outcome(&outcome),
    );
    urgency
}

fn urgency_of(key: &str) -> Option<Urgency> {
    match key {
        "low" => Some(Urgency::Low),
        "normal" => Some(Urgency::Normal),
        "high" => Some(Urgency::High),
        _ => None,
    }
}

#[cfg(test)]
#[path = "inbox_priority_tests.rs"]
mod tests;
