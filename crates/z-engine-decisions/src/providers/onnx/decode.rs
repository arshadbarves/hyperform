//! From one row of the model's logits to an answer: the checkpoint's
//! temperature for the question's type and option count, a softmax over the
//! option markers, and Laya's calibrated confidence `max(p)`.
//!
//! Ported from Laya's `laya/onnx_agent.py` (`_decode_answers`) and
//! `laya/common.py` (`temp_bucket`, `clamp_temperature`,
//! `answer_confidence`), <https://github.com/NandhaKishorM/laya>, Copyright
//! Convai Innovations, Apache-2.0. Changes: a score is reported from 0 to 1
//! (Laya's expected level divided by the highest level), and per-language
//! temperatures are left out because no request names a language.

use std::collections::BTreeMap;

use serde_json::Value;

use super::NAME;
use super::sequence::{LayaQuestion, QType};
use crate::answer::{AbstainReason, Answer, Verdict};
use crate::question::{Form, Question};

/// Laya refuses temperatures outside this range: below it a checkpoint's
/// fitted value would sharpen a coin flip into a certainty.
const TEMP_MIN: f64 = 0.5;
const TEMP_MAX: f64 = 5.0;

/// The checkpoint's temperatures, clamped as Laya clamps them.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Temperatures {
    /// By `QType` (choice, score, noul).
    by_type: [f64; 3],
    /// By bucket, e.g. `choice:3-5`.
    by_options: BTreeMap<String, f64>,
}

impl Temperatures {
    /// From the config's `temperature` list and `temperature_by_options`
    /// map; a missing or malformed list means 1.0 for every type.
    pub(super) fn from_config(temperature: Option<&Value>, by_options: Option<&Value>) -> Self {
        let mut by_type = [1.0; 3];
        if let Some(Value::Array(values)) = temperature {
            if values.len() == 3 {
                for (slot, value) in by_type.iter_mut().zip(values) {
                    *slot = clamp_temperature(value);
                }
            }
        }
        let by_options = match by_options {
            Some(Value::Object(map)) => map
                .iter()
                .map(|(bucket, value)| (bucket.clone(), clamp_temperature(value)))
                .collect(),
            _ => BTreeMap::new(),
        };
        Self {
            by_type,
            by_options,
        }
    }

    fn for_question(&self, kind: QType, options: usize) -> f64 {
        let bucket = temp_bucket(kind, options);
        let by_type = self.by_type[kind as usize];
        self.by_options.get(&bucket).copied().unwrap_or(by_type)
    }
}

/// A usable temperature: confined to the range, 1.0 if it is not a number.
fn clamp_temperature(value: &Value) -> f64 {
    match value.as_f64() {
        Some(t) if t.is_finite() => t.clamp(TEMP_MIN, TEMP_MAX),
        _ => 1.0,
    }
}

fn temp_bucket(kind: QType, options: usize) -> String {
    let size = match options {
        0..=2 => "2",
        3..=5 => "3-5",
        6..=10 => "6-10",
        _ => "11+",
    };
    format!("{}:{size}", kind.name())
}

/// Softmax of the logits divided by `temperature`, in `f32` as numpy
/// computes it over the model's `float32` output.
fn probabilities(logits: &[f32], temperature: f64) -> Vec<f64> {
    let temperature = temperature as f32;
    let scaled: Vec<f32> = logits.iter().map(|z| z / temperature).collect();
    let max = scaled.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp: Vec<f32> = scaled.iter().map(|z| (z - max).exp()).collect();
    let total: f32 = exp.iter().sum();
    exp.into_iter().map(|p| f64::from(p / total)).collect()
}

/// Python's `round(value, 4)`: correctly rounded, as Rust's formatting is.
fn round4(value: f64) -> f64 {
    format!("{value:.4}").parse().unwrap_or(value)
}

/// The first index of the largest value, as numpy's `argmax`.
fn argmax(values: &[f64]) -> usize {
    let mut best = 0;
    for (index, value) in values.iter().enumerate() {
        if *value > values[best] {
            best = index;
        }
    }
    best
}

/// One answer from the question's row of logits (one per option marker).
pub(super) fn decode(
    question: &Question,
    laya: &LayaQuestion,
    logits: &[f32],
    temperatures: &Temperatures,
) -> Answer {
    let k = laya.options.len().min(logits.len());
    if k == 0 {
        return Answer::abstained(&question.name, AbstainReason::Malformed, NAME);
    }
    let p = probabilities(&logits[..k], temperatures.for_question(laya.kind, k));
    let best = argmax(&p);
    let proposal = match &question.form {
        Form::Choice(_) => Verdict::Choice(laya.keys[best].clone()),
        Form::YesNo { .. } => Verdict::YesNo(best == 0),
        Form::Score(_) => {
            let expected: f64 = p.iter().enumerate().map(|(i, p)| i as f64 * p).sum();
            let highest = k.saturating_sub(1).max(1) as f64;
            Verdict::Score((round4(expected) / highest).clamp(0.0, 1.0))
        }
    };
    let probabilities = laya
        .keys
        .iter()
        .zip(&p)
        .map(|(key, p)| (key.clone(), round4(*p)))
        .collect();
    Answer {
        question: question.name.clone(),
        proposal: Some(proposal),
        probabilities,
        confidence: Some(round4(p[best]).clamp(0.0, 1.0)),
        abstain: None,
        provider: NAME.to_string(),
        latency_ms: 0,
        cached: false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn temperatures_are_clamped_and_bucketed() {
        let temperatures = Temperatures::from_config(
            Some(&json!([2.0, "x", 9.0])),
            Some(&json!({ "choice:11+": 0.1, "choice:3-5": 1.5 })),
        );
        assert_eq!(temperatures.for_question(QType::Choice, 2), 2.0);
        assert_eq!(temperatures.for_question(QType::Choice, 4), 1.5);
        assert_eq!(temperatures.for_question(QType::Choice, 12), TEMP_MIN);
        assert_eq!(temperatures.for_question(QType::Score, 3), 1.0);
        let missing = Temperatures::from_config(None, Some(&json!([])));
        assert_eq!(missing.for_question(QType::Score, 7), 1.0);
        assert_eq!(temp_bucket(QType::Score, 7), "score:6-10");
    }

    #[test]
    fn decoding_follows_the_options_and_reports_max_p() {
        let question = Question::yes_no("q", "Ok?\n- yes: Yes.\n- no: No.\n").unwrap();
        let laya = LayaQuestion::of(&question);
        let flat = Temperatures::from_config(None, None);
        let answer = decode(&question, &laya, &[0.0, 2.0, -1e4], &flat);
        assert_eq!(answer.yes(), Some(false));
        assert_eq!(answer.probabilities["no"], 0.8808);
        assert_eq!(answer.confidence, Some(0.8808));
        let score = Question::score("s", "S?\n- lo: L.\n- mid: M.\n- hi: H.\n").unwrap();
        let laya = LayaQuestion::of(&score);
        let answer = decode(&score, &laya, &[0.0, 0.0, 0.0], &flat);
        assert_eq!(answer.score(), Some(0.5));
        assert_eq!(answer.probabilities.len(), 3);
    }
}
