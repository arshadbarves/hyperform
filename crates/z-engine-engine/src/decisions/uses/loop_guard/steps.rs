//! One finished call as the guard remembers it: hashes of the call and of
//! its result, the failure's hash and class when it failed, whether it
//! edited files, and short labels for the decision model. Numbers are
//! masked before hashing, so timings and shifted line numbers still match.
//! Traces only ever see the hashes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use z_engine_protocol::{ToolResultPart, ToolStatus};
use z_engine_tools::names;

use crate::batch::ToolCall;
use crate::decisions::uses::digest::{head, input_digest};

const EDITS: &[&str] = &[
    names::WRITE,
    names::EDIT,
    names::MULTI_EDIT,
    names::NOTEBOOK_EDIT,
    names::APPLY_AGENT_CHANGES,
];
/// Polled or user-facing tools: repeating them is expected.
const POLLED: &[&str] = &[
    names::JOB_OUTPUT,
    names::TODO_WRITE,
    names::ASK_USER_QUESTION,
    names::EXIT_PLAN_MODE,
];
const ERROR_MARKERS: &[&str] = &["error", "fail", "panic", "assert", "exception", "denied"];
const ERROR_LINES: usize = 8;
const TAIL_LINES: usize = 12;
const LABEL_CHARS: usize = 120;
const EXCERPT_CHARS: usize = 300;

/// Lowercase markers, checked in this order: a missing tool beats a
/// timeout, which beats a code error.
const ENVIRONMENT: &[&str] = &[
    "command not found",
    "is not recognized as",
    "no such file or directory",
    "permission denied",
    "cannot find module",
    "no module named",
    "connection refused",
    "could not resolve host",
    "address already in use",
    "is not installed",
];
const RETRY: &[&str] = &[
    "timed out",
    "rate limit",
    "too many requests",
    "temporarily unavailable",
    "service unavailable",
    "connection reset",
    "resource busy",
    "try again",
];
const CODE: &[&str] = &[
    "error[e",
    "assert",
    "panicked",
    "typeerror",
    "syntaxerror",
    "referenceerror",
    "mismatched types",
    "test result: failed",
    "tests failed",
    "expected",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FailureClass {
    Retry,
    Code,
    Environment,
}

impl FailureClass {
    pub(super) fn parse(key: &str) -> Option<Self> {
        match key {
            "retry" => Some(Self::Retry),
            "code" => Some(Self::Code),
            "environment" => Some(Self::Environment),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Failure {
    pub hash: u64,
    /// From the markers; `None` when they say nothing.
    pub class: Option<FailureClass>,
    /// The error lines, for the decision model.
    pub excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Step {
    pub tool: String,
    pub call: u64,
    pub result: u64,
    pub failure: Option<Failure>,
    pub edit: bool,
    pub polled: bool,
    /// "Bash `cargo test` -> failed: ...", for the progress digest.
    pub label: String,
    pub target: String,
}

/// `None` for cancelled calls, which say nothing about progress.
pub(super) fn step(call: &ToolCall, status: ToolStatus, output: &[ToolResultPart]) -> Option<Step> {
    if status == ToolStatus::Cancelled {
        return None;
    }
    let text = text_of(output);
    let failed = status != ToolStatus::Ok;
    let failure = failed.then(|| failure(&call.name, &text));
    let target = head(&input_digest(&call.name, &call.input), LABEL_CHARS);
    let ended = match &failure {
        Some(failure) => format!("failed: {}", head(&failure.excerpt, LABEL_CHARS)),
        None => "ok".to_string(),
    };
    Some(Step {
        tool: call.name.clone(),
        call: hash(&(call.name.as_str(), call.input.to_string())),
        result: hash(&normalize(&text)),
        edit: !failed && EDITS.contains(&call.name.as_str()),
        polled: POLLED.contains(&call.name.as_str()),
        label: format!("{} {target} -> {ended}", call.name),
        target,
        failure,
    })
}

fn failure(tool: &str, text: &str) -> Failure {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let marked: Vec<&str> = (lines.iter().copied())
        .filter(|line| {
            let line = line.to_lowercase();
            ERROR_MARKERS.iter().any(|marker| line.contains(marker))
        })
        .take(ERROR_LINES)
        .collect();
    let tail = lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n");
    let joined = if marked.is_empty() {
        tail.clone()
    } else {
        marked.join("\n")
    };
    Failure {
        hash: hash(&(tool, normalize(&joined))),
        class: classify(&format!("{joined}\n{tail}")),
        excerpt: head(&joined, EXCERPT_CHARS),
    }
}

pub(super) fn classify(text: &str) -> Option<FailureClass> {
    let text = text.to_lowercase();
    let has = |markers: &[&str]| markers.iter().any(|marker| text.contains(marker));
    if has(ENVIRONMENT) {
        Some(FailureClass::Environment)
    } else if has(RETRY) {
        Some(FailureClass::Retry)
    } else if has(CODE) {
        Some(FailureClass::Code)
    } else {
        None
    }
}

fn text_of(output: &[ToolResultPart]) -> String {
    let texts = output.iter().filter_map(|part| match part {
        ToolResultPart::Text { text } => Some(text.as_str()),
        _ => None,
    });
    texts.collect::<Vec<_>>().join("\n")
}

/// Digits masked and whitespace collapsed.
fn normalize(text: &str) -> String {
    let masked: String = text
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect();
    masked.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_protocol::CallId;

    use super::*;

    fn bash(command: &str) -> ToolCall {
        ToolCall {
            id: CallId::from("c"),
            name: names::BASH.into(),
            input: json!({ "command": command }),
        }
    }

    fn text(text: &str) -> Vec<ToolResultPart> {
        vec![ToolResultPart::Text { text: text.into() }]
    }

    #[test]
    fn the_same_failure_matches_across_timings_and_line_numbers() {
        let first = "test a ... FAILED\nassertion failed at src/lib.rs:41\nfinished in 0.42s";
        let again = "test a ... FAILED\nassertion failed at src/lib.rs:44\nfinished in 0.57s";
        let a = step(&bash("cargo test"), ToolStatus::Error, &text(first)).unwrap();
        let b = step(&bash("cargo test"), ToolStatus::Error, &text(again)).unwrap();
        assert_eq!(
            a.failure.as_ref().unwrap().hash,
            b.failure.as_ref().unwrap().hash
        );
        assert_eq!(a.failure.unwrap().class, Some(FailureClass::Code));
        let other = step(
            &bash("cargo test"),
            ToolStatus::Error,
            &text("error: linker failed"),
        );
        assert_ne!(
            b.failure.unwrap().hash,
            other.unwrap().failure.unwrap().hash
        );
    }

    #[test]
    fn markers_classify_environment_retry_and_code() {
        assert_eq!(
            classify("sh: pnpm: command not found"),
            Some(FailureClass::Environment)
        );
        assert_eq!(
            classify("Command timed out after 1000 ms"),
            Some(FailureClass::Retry)
        );
        assert_eq!(
            classify("error[E0308]: mismatched types"),
            Some(FailureClass::Code)
        );
        assert_eq!(classify("Exit code 2"), None);
    }

    #[test]
    fn edits_count_only_when_they_succeed_and_cancelled_calls_are_skipped() {
        let edit = ToolCall {
            id: CallId::from("e"),
            name: names::EDIT.into(),
            input: json!({ "file_path": "src/lib.rs" }),
        };
        assert!(step(&edit, ToolStatus::Ok, &text("ok")).unwrap().edit);
        assert!(
            !step(&edit, ToolStatus::Error, &text("no match"))
                .unwrap()
                .edit
        );
        assert!(step(&edit, ToolStatus::Cancelled, &[]).is_none());
    }
}
