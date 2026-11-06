//! The benchmark fixtures: one small hand-labeled JSONL file per decision
//! kind under `tests/fixtures/<kind>.jsonl`, synthetic text only. Each line
//! is `{ id, question, label, state }`; `question` names a prompt in
//! `z_engine_prompts::decisions::ALL`.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;
use z_engine_decisions::{Form, Question};

/// A decision kind: its fixture file, its question, and the labels that
/// make the engine act (the positive class for precision and recall).
pub struct Kind {
    pub file: &'static str,
    pub question: &'static str,
    pub act: &'static [&'static str],
}

pub const KINDS: &[Kind] = &[
    Kind {
        file: "compaction_relevance",
        question: "compaction_relevant",
        act: &["no"],
    },
    Kind {
        file: "task_boundary",
        question: "task_boundary",
        act: &["related", "unrelated"],
    },
    Kind {
        file: "exchange_needed",
        question: "exchange_needed",
        act: &["no"],
    },
    Kind {
        file: "risk",
        question: "risk_injection",
        act: &["yes"],
    },
    Kind {
        file: "completion_claims",
        question: "completion_claims_done",
        act: &["yes"],
    },
    Kind {
        file: "loop_progress",
        question: "loop_guard_progress",
        act: &["no"],
    },
];

#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub id: String,
    pub question: String,
    pub label: String,
    pub state: Value,
}

pub fn load(kind: &Kind) -> Vec<Example> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{}.jsonl", kind.file));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(n, line)| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{}:{}: {e}", path.display(), n + 1))
        })
        .collect()
}

/// The question as the engine asks it: yes/no when the prompt offers
/// exactly yes and no, else a choice.
pub fn question(name: &str) -> Question {
    let (_, template) = z_engine_prompts::decisions::ALL
        .iter()
        .find(|(known, _)| *known == name)
        .unwrap_or_else(|| panic!("no prompt named {name}"));
    Question::yes_no(name, template)
        .or_else(|_| Question::choice(name, template))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The option keys `question` accepts.
pub fn options(question: &Question) -> Vec<String> {
    match &question.form {
        Form::Choice(options) => options.iter().map(|o| o.key.clone()).collect(),
        _ => vec!["yes".into(), "no".into()],
    }
}
