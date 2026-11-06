//! Repeated-question check (`decisions_question_check`): before an
//! `AskUserQuestion` call reaches the user, whether the chat already
//! answered it. Each question is held against the recent user messages
//! that share words with it, one yes/no per pair. When every question has
//! a confident earlier answer, the model gets a tool result pointing at
//! those messages instead; if it asks the same question again (matched by
//! fingerprint), the user is asked as today.

use async_trait::async_trait;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_context::render_template;
use z_engine_decisions::{AbstainReason, Answer, Question as DecisionQuestion, UNCHANGED};
use z_engine_prompts::decisions::QUESTION_CHECK_ANSWERED;
use z_engine_prompts::reminders::QUESTION_ANSWERED;
use z_engine_protocol::Question;

use super::digest::head;
use super::guidance::{ask_each, content_words, fingerprint, overlap, user_messages};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

pub(super) const QUESTION: &str = "question_check_answered";
pub(super) const ASKED_AGAIN: &str = "asked again; reached the user";
const RECENT_MESSAGES: usize = 8;
const PER_QUESTION: usize = 2;
const MESSAGE_CHARS: usize = 500;
const EXCERPT_CHARS: usize = 200;

#[derive(Debug)]
pub(crate) struct QuestionCheck;

pub(crate) static QUESTION_CHECK: QuestionCheck = QuestionCheck;

#[async_trait]
impl DecisionUse for QuestionCheck {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsQuestionCheck
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::AskUser]
    }

    async fn ask_user(&self, cx: &UseContext, questions: &[Question]) -> Option<String> {
        check(cx, questions).await
    }
}

/// One question and an earlier user message that may answer it.
struct Pair {
    question: usize,
    ordinal: usize,
    message: String,
}

async fn check(cx: &UseContext, questions: &[Question]) -> Option<String> {
    let memo = cx.core.decisions.memo();
    let keys: Vec<String> = questions.iter().map(question_key).collect();
    if keys.iter().any(|key| memo.contains(cx.feature, key)) {
        let answer = Answer::abstained(QUESTION, AbstainReason::Rules, "rules");
        for key in keys.iter().filter(|key| memo.contains(cx.feature, key)) {
            cx.record(cx.record_of(&answer, key).outcome(ASKED_AGAIN));
        }
        return None;
    }
    let pairs = candidates(cx, questions)?;
    let template = DecisionQuestion::yes_no(QUESTION, QUESTION_CHECK_ANSWERED).ok()?;
    let states = pairs.iter().map(|pair| {
        json!({
            "question": described(&questions[pair.question]),
            "message": head(&pair.message, MESSAGE_CHARS),
        })
    });
    let answers = ask_each(cx, &template, states.collect()).await;
    let mut best: Vec<Option<(usize, f64)>> = vec![None; questions.len()];
    for (index, (pair, (_, answer))) in pairs.iter().zip(&answers).enumerate() {
        let confidence = answer.confidence.unwrap_or_default();
        let slot = &mut best[pair.question];
        if answer.yes() == Some(true) && slot.is_none_or(|(_, c)| confidence > c) {
            *slot = Some((index, confidence));
        }
    }
    let chosen: Option<Vec<usize>> = best.iter().map(|slot| slot.map(|(at, _)| at)).collect();
    for (index, (fingerprint, answer)) in answers.iter().enumerate() {
        let picked = chosen
            .as_ref()
            .is_some_and(|chosen| chosen.contains(&index));
        let outcome = if picked {
            format!("pointed to message {}", pairs[index].ordinal)
        } else {
            UNCHANGED.to_string()
        };
        cx.record(cx.record_of(answer, fingerprint).outcome(&outcome));
    }
    let chosen = chosen?;
    if cx.shadow {
        return None;
    }
    for key in &keys {
        memo.insert(cx.feature, key);
    }
    let lines: Vec<String> = chosen
        .iter()
        .map(|&index| {
            let pair = &pairs[index];
            let excerpt = head(pair.message.trim(), EXCERPT_CHARS);
            let asked = questions[pair.question].question.trim();
            format!("- \"{asked}\": message {} said \"{excerpt}\"", pair.ordinal)
        })
        .collect();
    Some(render_template(
        QUESTION_ANSWERED,
        &[("answers", &lines.join("\n"))],
    ))
}

/// For each question, the recent user messages sharing the most words with
/// it; `None` when one question has none, since then the user is asked.
fn candidates(cx: &UseContext, questions: &[Question]) -> Option<Vec<Pair>> {
    let messages = cx.core.with_state(|state| user_messages(&state.transcript));
    let skip = messages.len().saturating_sub(RECENT_MESSAGES);
    let recent = &messages[skip..];
    let mut pairs = Vec::new();
    for (index, question) in questions.iter().enumerate() {
        let words = content_words(&described(question));
        let mut ranked: Vec<(usize, &(usize, String))> = recent
            .iter()
            .map(|message| (overlap(&words, &content_words(&message.1)), message))
            .filter(|(shared, _)| *shared > 0)
            .collect();
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.0.cmp(&a.1.0)));
        ranked.truncate(PER_QUESTION);
        if ranked.is_empty() {
            return None;
        }
        pairs.extend(ranked.into_iter().map(|(_, (ordinal, message))| Pair {
            question: index,
            ordinal: *ordinal,
            message: message.clone(),
        }));
    }
    Some(pairs)
}

/// The question with its option labels, as the decision model reads it.
fn described(question: &Question) -> String {
    let labels: Vec<&str> = question.options.iter().map(|o| o.label.trim()).collect();
    format!(
        "{} Options: {}.",
        question.question.trim(),
        labels.join("; ")
    )
}

/// Matches a question asked again, however it is spaced or capitalized.
pub(super) fn question_key(question: &Question) -> String {
    let words: Vec<String> = question
        .question
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    fingerprint(json!(words.join(" ")))
}

#[cfg(test)]
#[path = "question_check_tests.rs"]
mod tests;
