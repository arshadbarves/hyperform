//! Standing-rule suggestions (`decisions_memory_suggest`): at turn start,
//! whether the message sets a rule for the future ("always use pnpm"). Only
//! sentences with a rule-like marker are asked about, one yes/no each; the
//! most confident yes offers a card to save it to the instruction files
//! (the Memory tab's write path, run by the app). Nothing is saved without
//! a click, and a sentence the instructions already hold is not offered.

use async_trait::async_trait;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_decisions::{Question, UNCHANGED};
use z_engine_prompts::decisions::MEMORY_SUGGEST_RULE;
use z_engine_protocol::decisions::SuggestionKind;

use super::digest::head;
use super::guidance::{ask_each, fingerprint, sentences};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::decisions::suggest::offer;

pub(super) const QUESTION: &str = "memory_suggest_rule";
const MESSAGE_CHARS: usize = 600;
const MAX_RULE_CHARS: usize = 200;
const MAX_ASKED: usize = 3;

/// Words that start most standing rules; other sentences are not asked about.
const MARKERS: &[&str] = &[
    "always",
    "never",
    "from now on",
    "going forward",
    "in the future",
    "every time",
    "whenever",
    "remember to",
    "make sure to",
    "don't ever",
    "do not ever",
    "stop using",
    "by default",
    "as a rule",
    "prefer ",
];

#[derive(Debug)]
pub(crate) struct MemorySuggest;

pub(crate) static MEMORY_SUGGEST: MemorySuggest = MemorySuggest;

#[async_trait]
impl DecisionUse for MemorySuggest {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsMemorySuggest
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
    let rules = rule_like(cx, text);
    if rules.is_empty() {
        return;
    }
    let Ok(question) = Question::yes_no(QUESTION, MEMORY_SUGGEST_RULE) else {
        return;
    };
    let message = head(text.trim(), MESSAGE_CHARS);
    let states = rules
        .iter()
        .map(|rule| json!({ "sentence": rule, "message": message }));
    let answers = ask_each(cx, &question, states.collect()).await;
    let best = answers
        .iter()
        .enumerate()
        .filter_map(|(index, (_, answer))| {
            let confidence = answer.confidence.unwrap_or_default();
            (answer.yes() == Some(true)).then_some((index, confidence))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index);
    for (index, (fingerprint, answer)) in answers.iter().enumerate() {
        let outcome = if best == Some(index) {
            "offered to save"
        } else {
            UNCHANGED
        };
        cx.record(cx.record_of(answer, fingerprint).outcome(outcome));
    }
    let Some(index) = best else {
        return;
    };
    let rule = rules[index].clone();
    let key = rule_key(&rule);
    cx.core.decisions.memo().insert(cx.feature, &key);
    offer(cx, &answers[index].0, SuggestionKind::SaveRule { rule });
}

/// Sentences with a marker, short enough to be one rule, not offered
/// before in this session and not already in the instruction files.
fn rule_like(cx: &UseContext, text: &str) -> Vec<String> {
    let settings = cx.core.settings();
    let known: Vec<String> = settings
        .instructions
        .iter()
        .map(|doc| doc.content.to_lowercase())
        .collect();
    let memo = cx.core.decisions.memo();
    let mut found: Vec<String> = sentences(text)
        .into_iter()
        .filter(|sentence| sentence.chars().count() <= MAX_RULE_CHARS)
        .filter(|sentence| {
            let lower = sentence.to_lowercase();
            MARKERS.iter().any(|marker| lower.contains(marker))
                && !known
                    .iter()
                    .any(|doc| doc.contains(lower.trim_end_matches('.')))
        })
        .filter(|sentence| !memo.contains(cx.feature, &rule_key(sentence)))
        .map(str::to_string)
        .collect();
    found.truncate(MAX_ASKED);
    found
}

fn rule_key(rule: &str) -> String {
    fingerprint(json!(rule.trim().to_lowercase()))
}

#[cfg(test)]
#[path = "memory_suggest_tests.rs"]
mod tests;
