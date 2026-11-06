//! The `POST /v1/systemone` wire format shared by laya-serve and Jev.
//! Unknown fields are ignored; `act_probability` carries no signal and is
//! never read.

use std::collections::BTreeMap;

use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::answer::{AbstainReason, Answer, Verdict};
use crate::question::{Criterion, Form, Question};

/// Yes/no questions go out as a two-option choice with neutral keys:
/// Laya's `noul` answers can follow option labels such as "yes".
pub(super) const YES_KEY: &str = "a";
pub(super) const NO_KEY: &str = "b";

/// One request body. laya-serve reads `model` (a checkpoint name or alias;
/// an unknown name auto-routes) and `max_len` (a positive integer within its
/// token budget) from the body, not from its environment.
#[derive(Debug, Serialize)]
pub(super) struct Body<'a> {
    state: &'a Value,
    questions: Questions<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_len: Option<u32>,
}

/// Request settings sent with every body.
#[derive(Debug, Clone, Copy)]
pub(super) struct Controls<'a> {
    pub checkpoint: &'a str,
    pub max_len: u32,
}

pub(super) fn encode<'a>(
    state: &'a Value,
    questions: &'a [&'a Question],
    controls: Controls<'a>,
) -> Body<'a> {
    let checkpoint = controls.checkpoint.trim();
    Body {
        state,
        questions: Questions(questions),
        model: (!checkpoint.is_empty()).then_some(checkpoint),
        max_len: (controls.max_len > 0).then_some(controls.max_len),
    }
}

/// Questions by name, in request order.
#[derive(Debug)]
struct Questions<'a>(&'a [&'a Question]);

impl Serialize for Questions<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for question in self.0 {
            map.serialize_entry(&question.name, &WireQuestion(question))?;
        }
        map.end()
    }
}

struct WireQuestion<'a>(&'a Question);

impl Serialize for WireQuestion<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let question = self.0;
        let kind = match question.form {
            Form::Score(_) => "score",
            Form::Choice(_) | Form::YesNo { .. } => "choice",
        };
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("type", kind)?;
        map.serialize_entry("instructions", &question.instructions)?;
        map.serialize_entry("criteria", &Criteria(&question.form))?;
        map.end()
    }
}

/// A choice's options as `{key: description}` in template order (Laya reads
/// them positionally); a score's levels as a list, lowest first.
struct Criteria<'a>(&'a Form);

impl Serialize for Criteria<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Form::Choice(options) => {
                let mut map = serializer.serialize_map(Some(options.len()))?;
                for option in options {
                    map.serialize_entry(&option.key, &option.description)?;
                }
                map.end()
            }
            Form::YesNo { yes, no } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry(YES_KEY, yes)?;
                map.serialize_entry(NO_KEY, no)?;
                map.end()
            }
            Form::Score(levels) => {
                serializer.collect_seq(levels.iter().map(|level| &level.description))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct WireResponse {
    pub answers: BTreeMap<String, WireAnswer>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct WireAnswer {
    #[serde(default)]
    pub choice: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub probabilities: Option<Value>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub answer_confidence: Option<f64>,
}

/// One answer per question; a missing or unusable entry abstains as
/// malformed without spoiling the others.
pub(super) fn decode(
    questions: &[&Question],
    response: &WireResponse,
    provider: &str,
) -> Vec<Answer> {
    questions
        .iter()
        .map(|question| {
            let decoded = response
                .answers
                .get(&question.name)
                .and_then(|wire| decode_answer(question, wire, provider));
            decoded.unwrap_or_else(|| {
                Answer::abstained(&question.name, AbstainReason::Malformed, provider)
            })
        })
        .collect()
}

pub(super) fn decode_answer(
    question: &Question,
    wire: &WireAnswer,
    provider: &str,
) -> Option<Answer> {
    let raw = probabilities(wire.probabilities.as_ref());
    let (proposal, probabilities, chosen) = match &question.form {
        Form::Choice(options) => {
            let key = wire.choice.as_deref()?;
            options.iter().find(|option| option.key == key)?;
            (Verdict::Choice(key.to_string()), raw, Some(key.to_string()))
        }
        Form::YesNo { .. } => {
            let yes = match wire.choice.as_deref()? {
                YES_KEY => true,
                NO_KEY => false,
                _ => return None,
            };
            let renamed = raw
                .into_iter()
                .filter_map(|(key, p)| match key.as_str() {
                    YES_KEY => Some(("yes".to_string(), p)),
                    NO_KEY => Some(("no".to_string(), p)),
                    _ => None,
                })
                .collect();
            let chosen = if yes { "yes" } else { "no" };
            (Verdict::YesNo(yes), renamed, Some(chosen.to_string()))
        }
        Form::Score(levels) => {
            let (score, probabilities) = decode_score(levels, wire.score?, raw)?;
            (Verdict::Score(score), probabilities, None)
        }
    };
    let from_distribution = chosen.and_then(|key| probabilities.get(&key).copied());
    let confidence = wire
        .answer_confidence
        .or(wire.confidence)
        .or(from_distribution)
        .filter(|confidence| (0.0..=1.0).contains(confidence))?;
    Some(Answer {
        question: question.name.clone(),
        proposal: Some(proposal),
        probabilities,
        confidence: Some(confidence),
        abstain: None,
        provider: provider.to_string(),
        latency_ms: 0,
        cached: false,
    })
}

/// laya-serve's score is the expected level index, `Σ i·p_i` over levels
/// keyed `"0"`..`"k-1"`, so it runs from 0 to k-1; it is divided by k-1.
/// A reply on that scale is recognized by its index-keyed distribution or a
/// score above 1; any other score must already be within 0..1. Probabilities
/// are reported by level key.
fn decode_score(
    levels: &[Criterion],
    score: f64,
    raw: BTreeMap<String, f64>,
) -> Option<(f64, BTreeMap<String, f64>)> {
    let highest = levels.len().checked_sub(1).filter(|&highest| highest > 0)? as f64;
    let level_of = |key: &str| key.parse::<usize>().ok().and_then(|i| levels.get(i));
    let indexed = !raw.is_empty() && raw.keys().all(|key| level_of(key).is_some());
    if !score.is_finite() || score < 0.0 {
        return None;
    }
    if indexed || score > 1.0 {
        if score > highest {
            return None;
        }
        let probabilities = raw
            .iter()
            .filter_map(|(key, p)| Some((level_of(key)?.key.clone(), *p)))
            .collect();
        return Some((score / highest, probabilities));
    }
    let named = raw
        .into_iter()
        .filter(|(key, _)| levels.iter().any(|level| &level.key == key))
        .collect();
    Some((score, named))
}

/// Only `{ key: probability }` maps are read; anything else is ignored.
fn probabilities(value: Option<&Value>) -> BTreeMap<String, f64> {
    let Some(Value::Object(map)) = value else {
        return BTreeMap::new();
    };
    map.iter()
        .filter_map(|(key, p)| Some((key.clone(), p.as_f64().filter(|p| p.is_finite())?)))
        .collect()
}
