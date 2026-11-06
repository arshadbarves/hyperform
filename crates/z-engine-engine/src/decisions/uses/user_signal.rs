//! Correction signal (`decisions_user_signal`): at turn start, whether the
//! new message corrects the agent or sounds frustrated, read beside the
//! last sentence the agent wrote. A confident yes adds a reminder to
//! restate the plan and ask before large edits this turn; permissions and
//! the run itself never change.

use async_trait::async_trait;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_decisions::{DecisionRequest, Question, UNCHANGED};
use z_engine_prompts::decisions::USER_SIGNAL_CORRECTION;
use z_engine_prompts::reminders::CORRECTION_SIGNAL;

use super::digest::{head, latest_assistant_text};
use super::guidance::last_sentence;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

pub(super) const QUESTION: &str = "user_signal_correction";
const AGENT_CHARS: usize = 300;
const MESSAGE_CHARS: usize = 800;

#[derive(Debug)]
pub(crate) struct UserSignal;

pub(crate) static USER_SIGNAL: UserSignal = UserSignal;

#[async_trait]
impl DecisionUse for UserSignal {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsUserSignal
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::TurnStart]
    }

    async fn turn_start(&self, cx: &UseContext, text: &str) -> Vec<String> {
        let corrected = correcting(cx, text).await;
        corrected
            .then(|| CORRECTION_SIGNAL.to_string())
            .into_iter()
            .collect()
    }
}

/// A first message has nothing to correct, so it is not asked about.
async fn correcting(cx: &UseContext, text: &str) -> bool {
    let said = cx
        .core
        .with_state(|state| latest_assistant_text(&state.working));
    if said.trim().is_empty() || text.trim().is_empty() {
        return false;
    }
    let Ok(question) = Question::yes_no(QUESTION, USER_SIGNAL_CORRECTION) else {
        return false;
    };
    let state = json!({
        "agent_said": last_sentence(&said, AGENT_CHARS),
        "message": head(text.trim(), MESSAGE_CHARS),
    });
    let request = DecisionRequest::new(state).ask(question);
    let Some(answer) = cx.ask(&request).await.into_iter().next() else {
        return false;
    };
    let corrected = answer.yes() == Some(true);
    let outcome = if corrected { "reminded" } else { UNCHANGED };
    cx.record(
        cx.record_of(&answer, &request.fingerprint())
            .outcome(outcome),
    );
    corrected
}

#[cfg(test)]
mod tests {
    use z_engine_config::FeatureMode;
    use z_engine_decisions::DecisionRecord;
    use z_engine_protocol::{Message, PermissionMode};

    use super::*;
    use crate::decisions::seams::at_turn_start;
    use crate::decisions::uses::scripted::{Scripted, install, main_run, session, traced};

    const FEATURE: FeatureId = FeatureId::DecisionsUserSignal;
    const NO_NOT_THAT: &str = "No, I said the other file. Undo that.";

    fn after_a_reply(handle: &crate::session::SessionHandle) {
        handle.core.with_state(|state| {
            state.working.push(Message::user_text("fix the login form"));
            state
                .working
                .push(Message::assistant_text("Fixed. I edited signup.ts."));
        });
    }

    #[tokio::test]
    async fn a_confident_correction_adds_the_reminder_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, _) = session(dir.path()).await;
        let ctx = main_run(&handle);
        after_a_reply(&handle);
        install(
            &handle,
            FEATURE,
            FeatureMode::On,
            Scripted::default().yes(QUESTION, true),
        );
        at_turn_start(&ctx, NO_NOT_THAT).await;
        let reminders = handle.core.reminders.take(&ctx.spec.agent_id);
        assert_eq!(reminders.len(), 1);
        assert!(reminders[0].contains("AskUserQuestion"), "{reminders:?}");
        assert_eq!(handle.core.mode(), PermissionMode::Default);
        let records = handle.core.decisions.trace().recent(10);
        assert!(records.iter().any(|record| record.outcome == "reminded"));
        handle.close("test").await;
    }

    #[tokio::test]
    async fn no_off_down_unsure_and_shadow_add_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, _) = session(dir.path()).await;
        let ctx = main_run(&handle);
        after_a_reply(&handle);
        for (mode, model) in [
            (FeatureMode::On, Scripted::default().yes(QUESTION, false)),
            (FeatureMode::Off, Scripted::default().yes(QUESTION, true)),
            (FeatureMode::On, Scripted::down()),
            (FeatureMode::On, Scripted::default()),
            (FeatureMode::Shadow, Scripted::default().yes(QUESTION, true)),
        ] {
            install(&handle, FEATURE, mode, model);
            at_turn_start(&ctx, NO_NOT_THAT).await;
        }
        let would = |record: &DecisionRecord| record.shadow && record.outcome == "reminded";
        assert!(
            traced(&handle, would).await,
            "shadow records what it would do"
        );
        assert!(handle.core.reminders.take(&ctx.spec.agent_id).is_empty());
        handle.close("test").await;
    }

    #[tokio::test]
    async fn a_first_message_is_not_asked_about() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, _) = session(dir.path()).await;
        let ctx = main_run(&handle);
        install(
            &handle,
            FEATURE,
            FeatureMode::On,
            Scripted::default().yes(QUESTION, true),
        );
        at_turn_start(&ctx, NO_NOT_THAT).await;
        assert!(handle.core.reminders.take(&ctx.spec.agent_id).is_empty());
        assert!(handle.core.decisions.trace().recent(10).is_empty());
        handle.close("test").await;
    }
}
