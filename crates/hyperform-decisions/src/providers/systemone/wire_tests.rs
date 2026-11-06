//! The wire format: exact request bodies, laya-serve's score scale, and
//! Laya's own replies (`answers_multilingual.json`, ONNXAgent output in the
//! shape laya-serve returns) decoding the way the native runtime reports them.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::wire::{Controls, WireAnswer, WireResponse, decode, decode_answer, encode};
use crate::answer::{AbstainReason, Verdict};
use crate::providers::onnx::parity_tests::FixtureQuestion;
use crate::question::{Form, Question};

const ANSWERS: &str = include_str!("../../../tests/fixtures/onnx/answers_multilingual.json");

const CONTROLS: Controls<'static> = Controls {
    checkpoint: "multilingual",
    max_len: 1024,
};

fn yes_no() -> Question {
    Question::yes_no("relevant", "Relevant?\n- yes: Needed.\n- no: Not needed.\n").unwrap()
}

fn tool() -> Question {
    let template = "Which tool?\n- shell: Run it.\n- read: Read it.\n- edit: Change it.\n";
    Question::choice("tool", template).unwrap()
}

fn risk() -> Question {
    let template = "How risky?\n- none: Harmless.\n- some: Restorable.\n- high: Destructive.\n";
    Question::score("risk", template).unwrap()
}

#[test]
fn bodies_keep_template_order_and_send_model_and_max_len() {
    let (tool, risk, relevant) = (tool(), risk(), yes_no());
    let questions = [&tool, &risk, &relevant];
    let state = json!({ "z": 1, "a": 2 });
    let body = encode(&state, &questions, CONTROLS);
    let sent = serde_json::to_string(&body).unwrap();
    let want = concat!(
        r#"{"state":{"a":2,"z":1},"questions":{"#,
        r#""tool":{"type":"choice","instructions":"Which tool?","#,
        r#""criteria":{"shell":"Run it.","read":"Read it.","edit":"Change it."}},"#,
        r#""risk":{"type":"score","instructions":"How risky?","#,
        r#""criteria":["Harmless.","Restorable.","Destructive."]},"#,
        r#""relevant":{"type":"choice","instructions":"Relevant?","#,
        r#""criteria":{"a":"Needed.","b":"Not needed."}}},"#,
        r#""model":"multilingual","max_len":1024}"#,
    );
    assert_eq!(sent, want);
}

#[test]
fn an_empty_checkpoint_and_zero_max_len_are_left_to_the_server() {
    let relevant = yes_no();
    let questions = [&relevant];
    let controls = Controls {
        checkpoint: "  ",
        max_len: 0,
    };
    let state = json!({});
    let body = serde_json::to_value(encode(&state, &questions, controls)).unwrap();
    assert!(body.get("model").is_none());
    assert!(body.get("max_len").is_none());
}

fn score_reply(score: f64, probabilities: Value) -> WireAnswer {
    WireAnswer {
        score: Some(score),
        probabilities: Some(probabilities),
        answer_confidence: Some(0.5),
        ..WireAnswer::default()
    }
}

#[test]
fn scores_on_laya_index_scale_are_normalized_by_level_count() {
    let risk = risk();
    let decoded = |wire: WireAnswer| decode_answer(&risk, &wire, "s").map(|a| a.proposal);
    let laya = json!({ "0": 0.25, "1": 0.25, "2": 0.5 });
    // Laya: 0·0.25 + 1·0.25 + 2·0.5 = 1.25 of 2.
    let answer = decode_answer(&risk, &score_reply(1.25, laya.clone()), "s").unwrap();
    assert_eq!(answer.proposal, Some(Verdict::Score(0.625)));
    assert_eq!(answer.probabilities["high"], 0.5);
    assert_eq!(answer.probabilities["none"], 0.25);
    // Below 1 the index-keyed distribution still says which scale it is.
    let low = score_reply(0.5, json!({ "0": 0.6, "1": 0.3, "2": 0.1 }));
    assert_eq!(decoded(low), Some(Some(Verdict::Score(0.25))));
    // No distribution: above 1 can only be the index scale.
    assert_eq!(
        decoded(score_reply(1.5, Value::Null)),
        Some(Some(Verdict::Score(0.75)))
    );
    // No index-keyed distribution and within 0..1: already normalized.
    let named = score_reply(0.4, json!({ "none": 0.6, "high": 0.4 }));
    assert_eq!(decoded(named), Some(Some(Verdict::Score(0.4))));
}

#[test]
fn malformed_scores_are_still_rejected() {
    let risk = risk();
    let rejected = |wire: WireAnswer| decode_answer(&risk, &wire, "s").is_none();
    assert!(
        rejected(score_reply(2.5, Value::Null)),
        "past the last level"
    );
    assert!(rejected(score_reply(2.01, json!({ "0": 0.5, "2": 0.5 }))));
    assert!(rejected(score_reply(-0.1, Value::Null)));
    assert!(rejected(score_reply(f64::INFINITY, Value::Null)));
    assert!(rejected(WireAnswer::default()), "no score");
}

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    question: FixtureQuestion,
    answer: Map<String, Value>,
}

/// What the native runtime reports for the same Laya output (`decode.rs`).
#[test]
fn laya_replies_decode_like_the_native_runtime() {
    let fixture: Fixture = serde_json::from_str(ANSWERS).unwrap();
    for case in fixture.cases {
        let (name, laya) = (&case.name, &case.answer);
        let question = case.question.question();
        let wire: WireAnswer = serde_json::from_value(Value::Object(laya.clone())).unwrap();
        let answer = decode_answer(&question, &wire, "systemone").unwrap();
        let ours = |key: &str| match (&question.form, key) {
            (Form::YesNo { .. }, "a") => "yes".to_string(),
            (Form::YesNo { .. }, "b") => "no".to_string(),
            (Form::Score(levels), index) => levels[index.parse::<usize>().unwrap()].key.clone(),
            (_, key) => key.to_string(),
        };
        for (key, p) in laya["probabilities"].as_object().unwrap() {
            assert_eq!(
                answer.probabilities[&ours(key)],
                p.as_f64().unwrap(),
                "{name}: {key}"
            );
        }
        let expected = match &question.form {
            Form::YesNo { .. } => Verdict::YesNo(laya["choice"] == "a"),
            Form::Choice(_) => Verdict::Choice(laya["choice"].as_str().unwrap().into()),
            Form::Score(levels) => {
                Verdict::Score(laya["score"].as_f64().unwrap() / (levels.len() - 1) as f64)
            }
        };
        assert_eq!(answer.proposal, Some(expected), "{name}");
        assert_eq!(
            answer.confidence,
            laya["answer_confidence"].as_f64(),
            "{name}"
        );
    }
}

#[test]
fn answers_decode_and_unknown_fields_are_ignored() {
    let (relevant, tool) = (yes_no(), tool());
    let response: WireResponse = serde_json::from_value(json!({
        "answers": {
            "relevant": { "choice": "b", "probabilities": { "a": 0.1, "b": 0.9 },
                          "answer_confidence": 0.9, "act_probability": 0.99, "noul": "x" },
            "tool": { "choice": "green", "confidence": 0.99 }
        },
        "routing": { "anything": true }
    }))
    .unwrap();
    let answers = decode(&[&relevant, &tool], &response, "systemone");
    assert_eq!(answers[0].yes(), Some(false));
    assert_eq!(answers[0].probabilities.get("no"), Some(&0.9));
    assert_eq!(answers[0].confidence, Some(0.9));
    assert_eq!(answers[1].abstain, Some(AbstainReason::Malformed));
}

#[test]
fn confidence_falls_back_to_the_distribution_and_must_be_a_probability() {
    let tool = tool();
    let wire = |confidence: Option<f64>| WireAnswer {
        choice: Some("read".into()),
        probabilities: Some(json!({ "read": 0.7, "shell": 0.3 })),
        confidence,
        ..WireAnswer::default()
    };
    let answer = decode_answer(&tool, &wire(None), "s").unwrap();
    assert_eq!(answer.confidence, Some(0.7));
    assert!(decode_answer(&tool, &wire(Some(1.5)), "s").is_none());
}
