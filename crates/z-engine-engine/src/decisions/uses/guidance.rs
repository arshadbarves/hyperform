//! Helpers the guidance uses share (plan, correction, hints, standing
//! rules, repeated questions, review): the user's own messages, sentences,
//! content words for cheap deterministic prefilters, and one yes/no
//! question per candidate. The decision model is weak above about 20
//! options, so candidates are never listed in one question.

use std::collections::BTreeSet;

use futures::future::join_all;
use serde_json::Value;
use z_engine_decisions::{AbstainReason, Answer, DecisionRequest, Question};
use z_engine_protocol::{ContentBlock, Message, Role};

use crate::decisions::context::UseContext;

const REMINDER_OPEN: &str = "<system-reminder>";

const STOP_WORDS: &[&str] = &[
    "about", "after", "again", "all", "also", "and", "any", "are", "because", "been", "before",
    "but", "can", "could", "did", "does", "done", "for", "from", "get", "had", "has", "have",
    "here", "how", "into", "its", "just", "let", "like", "make", "more", "need", "not", "now",
    "one", "only", "our", "out", "please", "should", "some", "than", "that", "the", "their",
    "them", "then", "there", "these", "they", "this", "use", "using", "want", "was", "were",
    "what", "when", "where", "which", "while", "will", "with", "would", "you", "your",
];

/// The user's own messages in `transcript`, numbered from 1, without tool
/// rounds and without the reminders the engine appended.
pub(super) fn user_messages(transcript: &[Message]) -> Vec<(usize, String)> {
    let typed = transcript.iter().filter(|message| {
        let results = message.content.iter();
        message.role == Role::User
            && !results
                .clone()
                .any(|block| matches!(block, ContentBlock::ToolResult { .. }))
    });
    let texts = typed.map(|message| {
        let parts = message.content.iter().filter_map(|block| match block {
            ContentBlock::Text { text } if !text.trim_start().starts_with(REMINDER_OPEN) => {
                Some(text.trim())
            }
            _ => None,
        });
        parts.collect::<Vec<_>>().join("\n")
    });
    let numbered = texts.enumerate().map(|(index, text)| (index + 1, text));
    numbered.filter(|(_, text)| !text.is_empty()).collect()
}

/// Sentences of `text`: split after `.`, `!` and `?` and at line breaks.
pub(super) fn sentences(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, ch)) = chars.next() {
        let next_is_space = chars.peek().is_none_or(|(_, next)| next.is_whitespace());
        let end = match ch {
            '\n' => Some(at),
            '.' | '!' | '?' if next_is_space => Some(at + ch.len_utf8()),
            _ => None,
        };
        if let Some(end) = end {
            found.push(text[start..end].trim());
            start = end;
        }
    }
    found.push(text[start..].trim());
    found.retain(|sentence| !sentence.is_empty());
    found
}

/// The last sentence of `text`, cut to `chars` characters.
pub(super) fn last_sentence(text: &str, chars: usize) -> String {
    let last = sentences(text).pop().unwrap_or_default();
    super::digest::head(last, chars)
}

pub(super) fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Lowercase words of three or more characters that carry meaning, with a
/// plural `s` dropped so "migrations" meets "migration".
pub(super) fn content_words(text: &str) -> BTreeSet<String> {
    let lower = text.to_lowercase();
    let words = lower.split(|ch: char| !ch.is_alphanumeric());
    let kept = words.filter(|word| word.chars().count() >= 3 && !STOP_WORDS.contains(word));
    kept.map(|word| match word.strip_suffix('s') {
        Some(stem) if stem.len() > 3 && !stem.ends_with('s') => stem.to_string(),
        _ => word.to_string(),
    })
    .collect()
}

pub(super) fn overlap(a: &BTreeSet<String>, b: &BTreeSet<String>) -> usize {
    a.intersection(b).count()
}

/// A short digest of `value` for trace records and memo keys; the value
/// itself is never stored.
pub(super) fn fingerprint(value: Value) -> String {
    DecisionRequest::new(value).fingerprint()
}

/// Asks `question` about each state, concurrently, one request per state
/// (calibration is per question name, so every answer shares it). Answers
/// come back in state order with each request's fingerprint.
pub(super) async fn ask_each(
    cx: &UseContext,
    question: &Question,
    states: Vec<Value>,
) -> Vec<(String, Answer)> {
    let asks = states.into_iter().map(|state| async move {
        let request = DecisionRequest::new(state).ask(question.clone());
        let fingerprint = request.fingerprint();
        let answer = cx.ask(&request).await.into_iter().next();
        let provider = cx.service.provider_name();
        let answer = answer
            .unwrap_or_else(|| Answer::abstained(&question.name, AbstainReason::Invalid, provider));
        (fingerprint, answer)
    });
    join_all(asks).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_messages_skip_tool_rounds_and_reminders() {
        let transcript = vec![
            Message::user_text("use pnpm"),
            Message::assistant_text("ok"),
            Message::new(
                Role::User,
                vec![ContentBlock::tool_result("c1".into(), "done", false)],
            ),
            Message::new(
                Role::User,
                vec![
                    ContentBlock::text("now the parser"),
                    ContentBlock::text("<system-reminder>\nx\n</system-reminder>"),
                ],
            ),
        ];
        assert_eq!(
            user_messages(&transcript),
            [
                (1, "use pnpm".to_string()),
                (2, "now the parser".to_string())
            ]
        );
    }

    #[test]
    fn sentences_split_on_ends_and_lines() {
        let text = "Fix v1.2 first. Then run it!\nAlways use pnpm";
        assert_eq!(
            sentences(text),
            ["Fix v1.2 first.", "Then run it!", "Always use pnpm"]
        );
        assert_eq!(
            last_sentence("Done. Want more tests?", 100),
            "Want more tests?"
        );
        assert_eq!(last_sentence("", 10), "");
    }

    #[test]
    fn content_words_drop_noise_and_plurals() {
        let words = content_words("Please add the migrations for users, with tests");
        let expected = ["add", "migration", "test", "user"].map(String::from);
        assert_eq!(words, BTreeSet::from(expected));
        assert_eq!(overlap(&words, &content_words("a migration")), 1);
        assert_eq!(word_count(" a  b c "), 3);
    }
}
