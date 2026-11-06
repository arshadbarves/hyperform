//! Check selection (`decisions_check_select`): which automatic checks the
//! turn's changes can affect. A check whose ecosystem owns a changed file
//! always runs (path mapping decides first); the decision model is asked
//! only about the rest, one check per request. Anything short of a
//! confident "no" for every one of them keeps every check, and the seam
//! never narrows when a test file changed.

use std::path::PathBuf;

use async_trait::async_trait;
use futures::future::join_all;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_decisions::{Answer, DecisionRequest, Question, UNCHANGED};
use z_engine_prompts::decisions::CHECK_SELECT as TEMPLATE;
use z_engine_verify::{CheckSpec, owns_change};

use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

const QUESTION: &str = "check_select";
/// More undecided checks than this and every check runs.
const MAX_ASKED: usize = 8;
const FILES: usize = 20;

#[derive(Debug)]
pub(crate) struct CheckSelect;

pub(crate) static CHECK_SELECT: CheckSelect = CheckSelect;

#[async_trait]
impl DecisionUse for CheckSelect {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsCheckSelect
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::CheckSelect]
    }

    async fn check_select(
        &self,
        cx: &UseContext,
        checks: &[CheckSpec],
        changed: &[PathBuf],
    ) -> Vec<String> {
        let undecided: Vec<&CheckSpec> = checks
            .iter()
            .filter(|check| !owns_change(check, changed))
            .collect();
        if undecided.is_empty() || undecided.len() > MAX_ASKED {
            return Vec::new();
        }
        let Ok(question) = Question::yes_no(QUESTION, TEMPLATE) else {
            return Vec::new();
        };
        let files: Vec<String> = changed
            .iter()
            .take(FILES)
            .map(|path| path.display().to_string())
            .collect();
        let requests: Vec<DecisionRequest> = undecided
            .iter()
            .map(|check| {
                let state = json!({
                    "check": {
                        "label": check.label,
                        "kind": check.kind,
                        "command": check.command,
                        "folder": check.cwd.display().to_string(),
                    },
                    "changed_files": files,
                });
                DecisionRequest::new(state).ask(question.clone())
            })
            .collect();
        let answers = join_all(requests.iter().map(|request| cx.ask(request))).await;
        let verdicts: Vec<Option<bool>> = answers
            .iter()
            .map(|answers| answers.first().and_then(Answer::yes))
            .collect();
        let ids: Vec<&str> = undecided.iter().map(|check| check.id.as_str()).collect();
        let skip = skips(&ids, &verdicts);
        for ((request, answers), id) in requests.iter().zip(&answers).zip(&ids) {
            let outcome = if skip.iter().any(|s| s == id) {
                "skipped"
            } else {
                UNCHANGED
            };
            let fingerprint = request.fingerprint();
            for answer in answers {
                cx.record(cx.record_of(answer, &fingerprint).outcome(outcome));
            }
        }
        skip
    }
}

/// The ids answered with a confident "no", or none at all when any answer
/// was unsure.
fn skips(ids: &[&str], verdicts: &[Option<bool>]) -> Vec<String> {
    if ids.len() != verdicts.len() || verdicts.iter().any(Option::is_none) {
        return Vec::new();
    }
    ids.iter()
        .zip(verdicts)
        .filter(|(_, verdict)| **verdict == Some(false))
        .map(|(id, _)| id.to_string())
        .collect()
}

#[cfg(test)]
#[path = "check_select_tests.rs"]
mod tests;
