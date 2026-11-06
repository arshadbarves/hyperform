//! The port against fixtures from Laya's reference Python
//! (`tests/fixtures/onnx/generate.py` runs Laya's own `build_sequence`,
//! `render_options` and `_decode_answers`). Sequences use the generator's
//! stand-in tokenizer, reimplemented here.

use serde::Deserialize;
use serde_json::Value;

use super::decode::{Temperatures, decode};
use super::pyjson::serialize_state;
use super::sequence::{Budget, LayaQuestion, SpecialTokens, Tokenize, build_row, encode_state};
use crate::answer::Verdict;
use crate::question::{Criterion, Form, Question};

const SEQUENCES: &str = include_str!("../../../tests/fixtures/onnx/sequences.json");
const DECODING: &str = include_str!("../../../tests/fixtures/onnx/decoding.json");

/// `\s*\S+` pieces, each `10 + fnv1a32(piece) % 50000`.
struct FakeTok;

impl Tokenize for FakeTok {
    fn encode(&self, text: &str) -> Result<Vec<u32>, String> {
        let mut ids = Vec::new();
        let (mut piece, mut in_word) = (String::new(), false);
        for ch in text.chars() {
            if ch.is_whitespace() && in_word {
                ids.push(piece_id(&piece));
                piece.clear();
                in_word = false;
            }
            in_word |= !ch.is_whitespace();
            piece.push(ch);
        }
        if in_word {
            ids.push(piece_id(&piece));
        }
        Ok(ids)
    }
}

fn piece_id(piece: &str) -> u32 {
    let hash = piece.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    10 + hash % 50_000
}

fn special() -> SpecialTokens {
    SpecialTokens {
        cls: 1,
        sep: 2,
        mask: 3,
        pad: 0,
        mask_text: "[MASK]".into(),
    }
}

/// A question as the fixtures write it (also read by the SystemOne wire tests).
#[derive(Deserialize)]
pub(crate) struct FixtureQuestion {
    form: String,
    instructions: String,
    options: Vec<(String, String)>,
}

impl FixtureQuestion {
    pub(crate) fn question(&self) -> Question {
        let criteria = || {
            let options = self.options.iter().map(|(key, description)| Criterion {
                key: key.clone(),
                description: description.clone(),
            });
            options.collect()
        };
        let form = match self.form.as_str() {
            "yes_no" => Form::YesNo {
                yes: self.options[0].1.clone(),
                no: self.options[1].1.clone(),
            },
            "score" => Form::Score(criteria()),
            _ => Form::Choice(criteria()),
        };
        Question {
            name: "q".into(),
            instructions: self.instructions.clone(),
            form,
        }
    }
}

#[derive(Deserialize)]
struct SequenceCase {
    name: String,
    state: Value,
    question: FixtureQuestion,
    max_len: usize,
    head_max_len: usize,
    options: Vec<String>,
    state_text: String,
    state_ids: Vec<u32>,
    #[serde(default)]
    error: bool,
    #[serde(default)]
    ids: Vec<u32>,
    #[serde(default)]
    markers: Vec<usize>,
}

#[derive(Deserialize)]
struct Fixture<T> {
    cases: Vec<T>,
}

#[test]
fn sequences_match_laya_build_sequence() {
    let fixture: Fixture<SequenceCase> = serde_json::from_str(SEQUENCES).unwrap();
    assert!(fixture.cases.len() >= 10);
    for case in fixture.cases {
        let name = &case.name;
        let laya = LayaQuestion::of(&case.question.question());
        assert_eq!(laya.options, case.options, "{name}: rendered options");
        let state = serialize_state(&case.state);
        assert_eq!(state, case.state_text, "{name}: serialized state");
        let state_ids = encode_state(&FakeTok, &special(), &state).unwrap();
        assert_eq!(state_ids, case.state_ids, "{name}: state tokens");
        let budget = Budget {
            max_len: case.max_len,
            head_max_len: case.head_max_len,
        };
        let row = build_row(
            &FakeTok,
            &special(),
            &laya,
            &state_ids,
            budget,
            case.state.is_array(),
        );
        match row {
            Err(_) if case.error => {}
            Ok(row) if !case.error => {
                assert_eq!(row.ids, case.ids, "{name}: ids");
                assert_eq!(row.markers, case.markers, "{name}: markers");
            }
            other => panic!("{name}: expected error={}, got {other:?}", case.error),
        }
    }
}

#[derive(Deserialize)]
struct DecodingCase {
    name: String,
    question: FixtureQuestion,
    logits: Vec<f32>,
    markers: usize,
    temperature: Value,
    temperature_by_options: Value,
    answer: LayaAnswer,
}

#[derive(Deserialize)]
struct LayaAnswer {
    #[serde(default)]
    choice: Option<String>,
    #[serde(default)]
    score: Option<f64>,
    probabilities: serde_json::Map<String, Value>,
    answer_confidence: f64,
}

#[test]
fn decoding_matches_laya_decode_answers() {
    let fixture: Fixture<DecodingCase> = serde_json::from_str(DECODING).unwrap();
    assert!(fixture.cases.len() >= 7);
    for case in fixture.cases {
        let name = &case.name;
        let question = case.question.question();
        let laya = LayaQuestion::of(&question);
        let temperatures =
            Temperatures::from_config(Some(&case.temperature), Some(&case.temperature_by_options));
        let answer = decode(
            &question,
            &laya,
            &case.logits[..case.markers],
            &temperatures,
        );
        let want = &case.answer;
        // Laya keys a score's levels by index and a yes/no's options by label.
        let our_key = |key: &str| match (&question.form, key) {
            (Form::YesNo { .. }, "a") => "yes".to_string(),
            (Form::YesNo { .. }, "b") => "no".to_string(),
            (Form::Score(levels), index) => levels[index.parse::<usize>().unwrap()].key.clone(),
            (_, key) => key.to_string(),
        };
        for (key, p) in &want.probabilities {
            let got = answer.probabilities[&our_key(key)];
            assert_eq!(got, p.as_f64().unwrap(), "{name}: p[{key}]");
        }
        assert_eq!(
            answer.confidence,
            Some(want.answer_confidence),
            "{name}: confidence"
        );
        let highest = (case.markers - 1) as f64;
        let expected = match (&question.form, &want.choice, want.score) {
            (Form::YesNo { .. }, Some(choice), _) => Verdict::YesNo(choice == "a"),
            (_, Some(choice), _) => Verdict::Choice(choice.clone()),
            (_, None, Some(score)) => Verdict::Score(score / highest),
            _ => panic!("{name}: the fixture has no answer"),
        };
        assert_eq!(answer.proposal, Some(expected), "{name}: proposal");
    }
}
