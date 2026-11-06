//! Suggestion cards: a use in `on` mode offers an action
//! (`Event::Suggested`) and nothing happens until the user acts on it or
//! dismisses it (`Command::ResolveSuggestion`); the answer is traced, so
//! acceptance can be measured.

use z_engine_config::FeatureId;
use z_engine_decisions::{AbstainReason, Answer, DecisionRecord, Verdict};
use z_engine_protocol::Event;
use z_engine_protocol::decisions::{Suggestion, SuggestionKind};

use super::context::UseContext;
use crate::session::SessionCore;

/// Memo key set when the user dismisses a suggestion of a feature.
pub(crate) const DISMISSED: &str = "suggestion dismissed";
const RESPONSE: &str = "user_response";

/// Shows the card; in shadow mode it is not shown.
pub(crate) fn offer(cx: &UseContext, fingerprint: &str, kind: SuggestionKind) {
    if cx.shadow {
        return;
    }
    let suggestion = Suggestion {
        suggestion_id: format!("{}:{fingerprint}", cx.feature.as_str()),
        kind,
    };
    cx.core.events.emit(Event::Suggested { suggestion });
}

/// Traces the user's answer (provider `user`) and clears the card.
pub(crate) fn resolve_suggestion(core: &SessionCore, suggestion_id: &str, accepted: bool) {
    match suggestion_id
        .split_once(':')
        .and_then(|(feature, fingerprint)| Some((FeatureId::parse(feature)?, fingerprint)))
    {
        Some((feature, fingerprint)) => {
            let mut answer = Answer::abstained(RESPONSE, AbstainReason::Rules, "user");
            answer.proposal = Some(Verdict::YesNo(accepted));
            let outcome = if accepted { "accepted" } else { "dismissed" };
            let record = DecisionRecord::of(feature.as_str(), &answer, fingerprint, false);
            core.decisions.trace().record(record.outcome(outcome));
            if !accepted {
                core.decisions.memo().insert(feature, DISMISSED);
            }
        }
        None => tracing::debug!(suggestion_id, "a suggestion of no known feature"),
    }
    core.events.emit(Event::SuggestionResolved {
        suggestion_id: suggestion_id.to_string(),
        accepted,
    });
}
