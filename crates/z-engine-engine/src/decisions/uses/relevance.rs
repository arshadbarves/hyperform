//! Asking about each part of a long tool result (`decisions_output_trim`,
//! `decisions_search_rank`): one yes/no request per part, since each part
//! is its own state, a few at a time. The trace gets one record per
//! ranking, saying what was or would be left out.

use futures::stream::{self, StreamExt};
use serde_json::json;
use z_engine_decisions::{Answer, DecisionRequest, Question};
use z_engine_tools::RankRequest;

use super::digest::{head, latest_request};
use crate::decisions::context::UseContext;

/// Requests in flight at once for one ranking.
const AT_ONCE: usize = 8;
const REQUEST_CHARS: usize = 600;
const SUBJECT_CHARS: usize = 300;

/// What the model said about each part, and the trace record's inputs.
pub(super) struct Ranked {
    /// Per part: yes, no, or `None` when it abstained.
    pub verdicts: Vec<Option<bool>>,
    first: Option<(Answer, String)>,
}

impl Ranked {
    pub(super) fn ruled_out(&self) -> usize {
        self.verdicts.iter().filter(|v| **v == Some(false)).count()
    }

    pub(super) fn relevant(&self) -> usize {
        self.verdicts.iter().filter(|v| **v == Some(true)).count()
    }

    /// `None` when the model said nothing usable: keep today's result.
    pub(super) fn advice(self) -> Option<Vec<Option<bool>>> {
        self.verdicts
            .iter()
            .any(Option::is_some)
            .then_some(self.verdicts)
    }

    /// One trace record for the whole ranking, from its first answer.
    pub(super) fn record(&self, cx: &UseContext, outcome: &str, tokens: u64) {
        let Some((answer, fingerprint)) = &self.first else {
            return;
        };
        let record = cx.record_of(answer, fingerprint).outcome(outcome);
        cx.record(if tokens > 0 {
            record.saved(tokens)
        } else {
            record
        });
    }
}

/// Asks `name` about each item of `request`; `part` names the item in the
/// state (`part`, `result`). `None` when the question does not parse.
pub(super) async fn ask_each(
    cx: &UseContext,
    request: &RankRequest,
    name: &str,
    template: &str,
    part: &str,
) -> Option<Ranked> {
    let question = Question::yes_no(name, template).ok()?;
    let task = cx.core.with_state(|state| latest_request(&state.working));
    let task = head(&task, REQUEST_CHARS);
    let subject = head(&request.subject, SUBJECT_CHARS);
    let requests: Vec<DecisionRequest> = request
        .items
        .iter()
        .map(|item| {
            let state = json!({
                "request": task,
                "tool": request.tool,
                "subject": subject,
                part: item,
            });
            DecisionRequest::new(state).ask(question.clone())
        })
        .collect();
    let asks: Vec<_> = requests
        .iter()
        .map(|asked| async move { cx.ask(asked).await.into_iter().next() })
        .collect();
    let answers: Vec<Option<Answer>> = stream::iter(asks).buffered(AT_ONCE).collect().await;
    let verdicts = answers
        .iter()
        .map(|answer| answer.as_ref().and_then(Answer::yes))
        .collect();
    let first = answers
        .into_iter()
        .zip(&requests)
        .find_map(|(answer, asked)| Some((answer?, asked.fingerprint())));
    Some(Ranked { verdicts, first })
}

/// Rough tokens in `chars` characters of tool output.
pub(super) fn tokens_in(chars: usize) -> u64 {
    u64::try_from(chars / 4).unwrap_or(u64::MAX)
}
