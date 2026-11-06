//! The prefetch use: on a request of at least a few words, asks once per
//! candidate whether the task needs that file, reads the confident picks
//! within the `[decisions.prefetch]` budget and returns them as reminders;
//! at turn end it records the tool rounds before the first edit.

use async_trait::async_trait;
use futures::future::join_all;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_decisions::{Answer, DecisionRequest, Question, UNCHANGED};
use z_engine_prompts::decisions::PREFETCH_NEEDED;
use z_engine_protocol::TurnRecord;
use z_engine_protocol::decisions::TurnTone;

use super::attach::{eligible, read_within};
use super::candidates::{Candidate, candidates, changed_files, outline};
use super::metrics;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::decisions::uses::digest::head;

pub(super) const QUESTION: &str = "prefetch_needed";
const REQUEST_CHARS: usize = 600;
/// Shorter messages ("yes", "go on") do not start a task.
const MIN_WORDS: usize = 4;

#[derive(Debug)]
pub(crate) struct Prefetch;

pub(crate) static PREFETCH: Prefetch = Prefetch;

#[async_trait]
impl DecisionUse for Prefetch {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsPrefetch
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::TurnStart, Seam::TurnEnd]
    }

    async fn turn_start(&self, cx: &UseContext, text: &str) -> Vec<String> {
        prefetch(cx, text).await
    }

    async fn turn_end(&self, cx: &UseContext, turn: &TurnRecord) -> Option<TurnTone> {
        metrics::record(cx, turn);
        None
    }
}

async fn prefetch(cx: &UseContext, text: &str) -> Vec<String> {
    if text.split_whitespace().count() < MIN_WORDS {
        return Vec::new();
    }
    let found = eligible(&cx.core, find(cx, text)).await;
    let Ok(question) = Question::yes_no(QUESTION, PREFETCH_NEEDED) else {
        return Vec::new();
    };
    if found.is_empty() {
        return Vec::new();
    }
    let request = head(text, REQUEST_CHARS);
    let asks = found
        .iter()
        .map(|candidate| ask(cx, &question, &request, candidate));
    let answers = join_all(asks).await;
    let picks: Vec<Candidate> = found
        .iter()
        .zip(&answers)
        .filter(|(_, (answer, _))| answer.yes() == Some(true))
        .map(|(candidate, _)| candidate.clone())
        .collect();
    let budget = cx.core.settings().settings.decisions.prefetch.clone();
    let max_files = usize::try_from(budget.max_files).unwrap_or(usize::MAX);
    let attached = read_within(&cx.core, &picks, max_files, u64::from(budget.max_tokens)).await;
    let tokens: u64 = attached.iter().map(|file| file.tokens).sum();
    let files = metrics::files_label(attached.len());
    let outcome = match (attached.is_empty(), cx.shadow) {
        (true, _) => UNCHANGED.to_string(),
        (false, true) => format!("would attach {files} ({tokens} tokens)"),
        (false, false) => format!("attached {files} ({tokens} tokens)"),
    };
    if let Some((answer, fingerprint)) = answers.first() {
        cx.record(cx.record_of(answer, fingerprint).outcome(&outcome));
    }
    if cx.shadow {
        return Vec::new();
    }
    for file in &attached {
        if cx.core.main.files.record_read(&file.full).is_err() {
            tracing::debug!(path = %file.full.display(), "prefetched file not tracked as read");
        }
    }
    attached.into_iter().map(|file| file.note).collect()
}

/// Candidates for `text`; changed files only on a chat's first turn.
fn find(cx: &UseContext, text: &str) -> Vec<Candidate> {
    let first_turn = cx.core.with_state(|state| state.turns.is_empty());
    let map = cx.core.repo_map.current();
    let files = map.as_deref().map(outline).unwrap_or_default();
    let changed = match (first_turn, cx.core.git_info()) {
        (true, Some(git)) => changed_files(&git.status_short),
        _ => Vec::new(),
    };
    let root = &cx.core.root;
    candidates(text, &files, &changed, |path| root.join(path).is_file())
}

async fn ask(
    cx: &UseContext,
    question: &Question,
    request: &str,
    candidate: &Candidate,
) -> (Answer, String) {
    let state = json!({
        "request": request,
        "file": candidate.path,
        "why": candidate.why,
        "defines": candidate.defines,
    });
    let asked = DecisionRequest::new(state).ask(question.clone());
    let fingerprint = asked.fingerprint();
    let answer = cx.ask(&asked).await.into_iter().next();
    let provider = cx.service.provider_name();
    let reason = z_engine_decisions::AbstainReason::Invalid;
    let answer = answer.unwrap_or_else(|| Answer::abstained(QUESTION, reason, provider));
    (answer, fingerprint)
}
