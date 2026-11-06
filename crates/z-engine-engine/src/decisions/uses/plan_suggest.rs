//! Plan suggestion (`decisions_plan_suggest`): at turn start, at the Default
//! permission level and outside plan mode only, how large and risky the
//! request is. A confident `large` offers a card ("Plan first?") whose
//! button turns plan mode on through the usual switch; nothing changes
//! unless the user clicks.
//! A choice, not a score: the model's choices calibrate better. After one
//! dismissal the session gets no more suggestions.

use async_trait::async_trait;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_decisions::{DecisionRequest, Question, UNCHANGED};
use z_engine_prompts::decisions::PLAN_SUGGEST_SIZE;
use z_engine_protocol::PermissionMode;
use z_engine_protocol::decisions::SuggestionKind;

use super::digest::head;
use super::guidance::word_count;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::decisions::suggest::{DISMISSED, offer};

pub(super) const QUESTION: &str = "plan_suggest_size";
const REQUEST_CHARS: usize = 1200;
/// Shorter messages ("yes", "go on") never start a large change.
const MIN_WORDS: usize = 8;

#[derive(Debug)]
pub(crate) struct PlanSuggest;

pub(crate) static PLAN_SUGGEST: PlanSuggest = PlanSuggest;

#[async_trait]
impl DecisionUse for PlanSuggest {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsPlanSuggest
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::TurnStart]
    }

    async fn turn_start(&self, cx: &UseContext, text: &str) -> Vec<String> {
        suggest(cx, text).await;
        Vec::new()
    }
}

async fn suggest(cx: &UseContext, text: &str) {
    if cx.core.planning()
        || cx.core.mode() != PermissionMode::Default
        || word_count(text) < MIN_WORDS
        || cx.core.decisions.memo().contains(cx.feature, DISMISSED)
    {
        return;
    }
    let Ok(question) = Question::choice(QUESTION, PLAN_SUGGEST_SIZE) else {
        return;
    };
    let request = DecisionRequest::new(json!({ "request": head(text, REQUEST_CHARS) }));
    let request = request.ask(question);
    let fingerprint = request.fingerprint();
    let Some(answer) = cx.ask(&request).await.into_iter().next() else {
        return;
    };
    let large = answer.choice() == Some("large");
    let outcome = match (large, cx.shadow) {
        (false, _) => UNCHANGED,
        (true, true) => "would suggest plan mode",
        (true, false) => "suggested plan mode",
    };
    cx.record(cx.record_of(&answer, &fingerprint).outcome(outcome));
    if large {
        offer(cx, &fingerprint, SuggestionKind::PlanFirst);
    }
}

#[cfg(test)]
mod tests {
    use z_engine_config::FeatureMode;
    use z_engine_protocol::Event;

    use super::*;
    use crate::decisions::resolve_suggestion;
    use crate::decisions::seams::at_turn_start;
    use crate::decisions::uses::scripted::{Events, Scripted, install, main_run, session, traced};

    const FEATURE: FeatureId = FeatureId::DecisionsPlanSuggest;
    const LARGE: &str = "Rewrite the storage layer to use SQLite instead of JSON files everywhere";

    fn sized(key: &str) -> Scripted {
        Scripted::default().choice(QUESTION, key)
    }

    fn offers(events: &Events) -> Vec<String> {
        let events = events.lock().unwrap();
        let offered = events.iter().filter_map(|event| match event {
            Event::Suggested { suggestion } if suggestion.kind == SuggestionKind::PlanFirst => {
                Some(suggestion.suggestion_id.clone())
            }
            _ => None,
        });
        offered.collect()
    }

    #[tokio::test]
    async fn a_confident_large_request_offers_a_card_and_never_switches_modes() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, events) = session(dir.path()).await;
        let ctx = main_run(&handle);
        install(&handle, FEATURE, FeatureMode::On, sized("large"));
        at_turn_start(&ctx, LARGE).await;
        let ids = offers(&events);
        assert_eq!(ids.len(), 1);
        assert!(ids[0].starts_with("decisions_plan_suggest:"), "{ids:?}");
        assert_eq!(handle.core.mode(), PermissionMode::Default);
        assert!(handle.core.reminders.take(&ctx.spec.agent_id).is_empty());
        resolve_suggestion(&handle.core, &ids[0], true);
        let accepted = |r: &z_engine_decisions::DecisionRecord| r.outcome == "accepted";
        assert!(traced(&handle, accepted).await);
        assert_eq!(handle.core.mode(), PermissionMode::Default);
        handle.close("test").await;
    }

    #[tokio::test]
    async fn small_off_down_unsure_and_shadow_offer_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, events) = session(dir.path()).await;
        let ctx = main_run(&handle);
        install(&handle, FEATURE, FeatureMode::On, sized("small"));
        at_turn_start(&ctx, LARGE).await;
        install(&handle, FEATURE, FeatureMode::Off, sized("large"));
        at_turn_start(&ctx, LARGE).await;
        for model in [Scripted::down(), Scripted::default()] {
            install(&handle, FEATURE, FeatureMode::On, model);
            at_turn_start(&ctx, LARGE).await;
        }
        install(&handle, FEATURE, FeatureMode::Shadow, sized("large"));
        at_turn_start(&ctx, LARGE).await;
        let would = |r: &z_engine_decisions::DecisionRecord| {
            r.shadow && r.outcome == "would suggest plan mode"
        };
        assert!(
            traced(&handle, would).await,
            "shadow records what it would do"
        );
        assert!(offers(&events).is_empty());
        handle.close("test").await;
    }

    #[tokio::test]
    async fn never_in_plan_or_bypass_mode_nor_for_short_messages_or_after_a_dismissal() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, events) = session(dir.path()).await;
        let ctx = main_run(&handle);
        install(&handle, FEATURE, FeatureMode::On, sized("large"));
        handle.core.with_state(|state| state.planning = true);
        at_turn_start(&ctx, LARGE).await;
        handle.core.with_state(|state| state.planning = false);
        for mode in [PermissionMode::Bypass, PermissionMode::AcceptEdits] {
            handle.core.with_state(|state| state.mode = mode);
            at_turn_start(&ctx, LARGE).await;
        }
        handle
            .core
            .with_state(|state| state.mode = PermissionMode::Default);
        at_turn_start(&ctx, "yes, go ahead").await;
        assert!(offers(&events).is_empty());
        assert!(handle.core.decisions.trace().recent(10).is_empty());
        at_turn_start(&ctx, LARGE).await;
        let id = offers(&events).pop().unwrap();
        resolve_suggestion(&handle.core, &id, false);
        at_turn_start(&ctx, LARGE).await;
        assert_eq!(offers(&events).len(), 1, "no more offers after a dismissal");
        handle.close("test").await;
    }
}
