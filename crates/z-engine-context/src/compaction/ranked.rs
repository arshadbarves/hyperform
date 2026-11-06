//! Relevance-aware microcompaction: the same trigger and the same
//! candidates as [`plan_microcompact`](super::plan_microcompact), with the
//! decision model's verdict on each older result. Without a verdict a
//! result follows today's size rule, so no answer means no change.

use z_engine_protocol::{CallId, Message};

use super::micro::{ClearTarget, older_results};

/// Results the model judged unrelated are cleared from this fraction of
/// `min_chars` up.
const IRRELEVANT_FLOOR_DIVISOR: usize = 4;

/// Tool results to clear, oldest first. `protected(call)` results (the hard
/// keeps, see `protected_results`) are never cleared, nor are the newest
/// `keep_recent` results and results already cleared. Otherwise
/// `relevance(call)` is the model's verdict: `Some(true)` (still needed)
/// keeps the result, `Some(false)` (unrelated) clears it from a quarter of
/// `min_chars`, `None` clears it above `min_chars` as today.
pub fn plan_microcompact_ranked(
    messages: &[Message],
    keep_recent: usize,
    min_chars: usize,
    relevance: impl Fn(&CallId) -> Option<bool>,
    protected: impl Fn(&CallId) -> bool,
) -> Vec<ClearTarget> {
    let floor = min_chars / IRRELEVANT_FLOOR_DIVISOR;
    older_results(messages, keep_recent)
        .into_iter()
        .filter(|target| !protected(&target.call_id))
        .filter(|target| match relevance(&target.call_id) {
            Some(true) => false,
            Some(false) => target.chars > floor,
            None => target.chars > min_chars,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_protocol::{ContentBlock, Role};

    use super::*;
    use crate::compaction::plan_microcompact;

    fn round(id: &str, len: usize) -> [Message; 2] {
        let call = Message::new(
            Role::Assistant,
            vec![ContentBlock::ToolUse {
                id: CallId::from(id),
                name: "Read".into(),
                input: json!({"file_path": format!("/r/{id}.rs")}),
            }],
        );
        let result = Message::new(
            Role::User,
            vec![ContentBlock::tool_result(
                CallId::from(id),
                "x".repeat(len),
                false,
            )],
        );
        [call, result]
    }

    /// Results c1..c5 of 100 chars, except c3 (30 chars).
    fn history() -> Vec<Message> {
        let mut messages = vec![Message::user_text("read things")];
        for (id, len) in [
            ("c1", 100),
            ("c2", 100),
            ("c3", 30),
            ("c4", 100),
            ("c5", 100),
        ] {
            messages.extend(round(id, len));
        }
        messages
    }

    fn ids(targets: &[ClearTarget]) -> Vec<&str> {
        targets
            .iter()
            .map(|target| target.call_id.as_str())
            .collect()
    }

    #[test]
    fn without_verdicts_it_is_todays_plan() {
        let messages = history();
        let ranked = plan_microcompact_ranked(&messages, 1, 50, |_| None, |_| false);
        assert_eq!(ranked, plan_microcompact(&messages, 1, 50));
    }

    #[test]
    fn relevant_results_stay_and_unrelated_small_ones_go() {
        let messages = history();
        let verdict = |id: &CallId| match id.as_str() {
            "c1" => Some(true),
            "c3" => Some(false),
            _ => None,
        };
        let ranked = plan_microcompact_ranked(&messages, 1, 50, verdict, |_| false);
        assert_eq!(ids(&ranked), ["c2", "c3", "c4"]);
    }

    #[test]
    fn protected_and_recent_results_are_never_cleared() {
        let messages = history();
        let ranked =
            plan_microcompact_ranked(&messages, 2, 50, |_| Some(false), |id| id.as_str() == "c2");
        assert_eq!(ids(&ranked), ["c1", "c3"]);
        let unjudged =
            plan_microcompact_ranked(&messages, 1, 50, |_| None, |id| id.as_str() == "c1");
        assert_eq!(ids(&unjudged), ["c2", "c4"]);
    }

    #[test]
    fn tiny_unrelated_results_stay() {
        let messages = history();
        let ranked = plan_microcompact_ranked(&messages, 0, 200, |_| Some(false), |_| false);
        assert_eq!(ids(&ranked), ["c1", "c2", "c4", "c5"]);
    }
}
