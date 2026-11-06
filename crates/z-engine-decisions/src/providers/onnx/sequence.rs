//! Laya's token sequence: how a state and one question become one row of
//! the model's input, `[CLS] <type> question: <instructions> [SEP] [MASK]
//! opt0 [MASK] opt1 ... [SEP] state [SEP]`, with one marker per `[MASK]`.
//!
//! Ported from Laya's `laya/common.py` (`render_options`, `build_sequence`)
//! and `laya/onnx_agent.py` (`_encode_state`),
//! <https://github.com/NandhaKishorM/laya>, Copyright Convai Innovations,
//! Apache-2.0. Changes: questions come from z-engine's `Question`, with a
//! yes/no question sent as a two-option choice under the neutral labels
//! `a`/`b` exactly as the SystemOne wire sends it; option reordering and the
//! statistics Laya reports alongside the sequence are left out.

use crate::question::{Form, Question};

/// Laya caps every option's text at this many tokens before budgeting.
const MAX_OPTION_TOKENS: usize = 48;
/// Labels a yes/no question's options carry in the sequence.
const YES_LABEL: &str = "a";
const NO_LABEL: &str = "b";

/// Laya's question types; the discriminant is the model's `qtype` input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QType {
    Choice = 0,
    Score = 1,
}

impl QType {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Choice => "choice",
            Self::Score => "score",
        }
    }
}

/// A question as Laya reads it: rendered option texts, and the answer key
/// each option stands for (`yes`/`no` for a yes/no question).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LayaQuestion {
    pub kind: QType,
    pub instructions: String,
    pub keys: Vec<String>,
    pub options: Vec<String>,
}

impl LayaQuestion {
    pub(super) fn of(question: &Question) -> Self {
        let (kind, pairs): (QType, Vec<(String, String)>) = match &question.form {
            Form::Choice(options) => {
                let pairs = options.iter().map(|option| {
                    let text = choice_text(&option.key, &option.description);
                    (option.key.clone(), text)
                });
                (QType::Choice, pairs.collect())
            }
            Form::YesNo { yes, no } => {
                let pairs = vec![
                    ("yes".to_string(), choice_text(YES_LABEL, yes)),
                    ("no".to_string(), choice_text(NO_LABEL, no)),
                ];
                (QType::Choice, pairs)
            }
            Form::Score(levels) => {
                let pairs = levels.iter().enumerate().map(|(index, level)| {
                    let text = format!("level {index}: {}", level.description);
                    (level.key.clone(), text)
                });
                (QType::Score, pairs.collect())
            }
        };
        let (keys, options) = pairs.into_iter().unzip();
        Self {
            kind,
            instructions: question.instructions.clone(),
            keys,
            options,
        }
    }
}

/// `render_options` for a choice: a label without a description is itself.
fn choice_text(key: &str, description: &str) -> String {
    if description.is_empty() {
        key.to_string()
    } else {
        format!("{key}: {description}")
    }
}

/// The checkpoint's tokenizer, as Laya calls it: no special tokens added.
pub(super) trait Tokenize {
    fn encode(&self, text: &str) -> Result<Vec<u32>, String>;
}

/// Ids and text of the special tokens Laya assembles sequences with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SpecialTokens {
    pub cls: u32,
    pub sep: u32,
    pub mask: u32,
    pub pad: u32,
    /// Replaced by a space in every text, so only markers are masks.
    pub mask_text: String,
}

/// Sequence length per question, and the part the question and options may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Budget {
    pub max_len: usize,
    pub head_max_len: usize,
}

/// One question's row: token ids and the positions of its option markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Row {
    pub ids: Vec<u32>,
    pub markers: Vec<usize>,
}

/// The state's tokens, shared by every question about it.
pub(super) fn encode_state(
    tok: &dyn Tokenize,
    special: &SpecialTokens,
    text: &str,
) -> Result<Vec<u32>, String> {
    tok.encode(&text.replace(&special.mask_text, " "))
}

/// `build_sequence`. A list state keeps its newest tokens (`truncate_left`);
/// any other keeps its first. Fails when an option's marker does not fit.
pub(super) fn build_row(
    tok: &dyn Tokenize,
    special: &SpecialTokens,
    question: &LayaQuestion,
    state_ids: &[u32],
    budget: Budget,
    truncate_left: bool,
) -> Result<Row, String> {
    let mask = special.mask_text.as_str();
    let instructions = question.instructions.replace(mask, " ");
    let head_ids = tok.encode(&format!(
        "{} question: {instructions}",
        question.kind.name()
    ))?;
    let mut option_ids = Vec::with_capacity(question.options.len());
    for option in &question.options {
        let mut tokens = tok.encode(&format!(" {}", option.replace(mask, " ")))?;
        tokens.truncate(MAX_OPTION_TOKENS);
        let mut ids = vec![special.mask];
        ids.extend(tokens);
        option_ids.push(ids);
    }
    let head_max_len = budget.head_max_len;
    let mut option_budget = signed(head_max_len) - total_len(&option_ids);
    if option_budget < 16 {
        let per = (head_max_len.saturating_sub(16) / option_ids.len().max(1)).max(4);
        option_ids.iter_mut().for_each(|ids| ids.truncate(per));
        option_budget = signed(head_max_len) - total_len(&option_ids);
    }
    let head_keep = usize::try_from(option_budget.max(8)).unwrap_or(8);
    let mut ids = vec![special.cls];
    ids.extend(head_ids.iter().take(head_keep));
    ids.push(special.sep);
    let mut markers = Vec::with_capacity(option_ids.len());
    for option in option_ids {
        markers.push(ids.len());
        ids.extend(option);
    }
    ids.push(special.sep);
    let room = budget.max_len.saturating_sub(ids.len() + 1);
    let state = if truncate_left {
        &state_ids[state_ids.len().saturating_sub(room)..]
    } else {
        &state_ids[..room.min(state_ids.len())]
    };
    ids.extend_from_slice(state);
    ids.push(special.sep);
    ids.truncate(budget.max_len);
    markers.retain(|marker| *marker < budget.max_len);
    if markers.len() != question.options.len() {
        return Err(format!(
            "only {} of its {} option markers fit in max_len={} with head_max_len={}",
            markers.len(),
            question.options.len(),
            budget.max_len,
            head_max_len
        ));
    }
    Ok(Row { ids, markers })
}

fn signed(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn total_len(rows: &[Vec<u32>]) -> i64 {
    signed(rows.iter().map(Vec::len).sum())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yes_no_is_a_neutral_choice_and_scores_name_levels() {
        let yes_no = Question::yes_no("q", "Relevant?\n- yes: Needed.\n- no: Not.\n").unwrap();
        let laya = LayaQuestion::of(&yes_no);
        assert_eq!(laya.kind, QType::Choice);
        assert_eq!(laya.keys, ["yes", "no"]);
        assert_eq!(laya.options, ["a: Needed.", "b: Not."]);
        let score = Question::score("s", "Size?\n- small: Small.\n- large: Large.\n").unwrap();
        let laya = LayaQuestion::of(&score);
        assert_eq!((laya.kind, laya.kind as i64), (QType::Score, 1));
        assert_eq!(laya.options, ["level 0: Small.", "level 1: Large."]);
        assert_eq!(laya.keys, ["small", "large"]);
    }
}
