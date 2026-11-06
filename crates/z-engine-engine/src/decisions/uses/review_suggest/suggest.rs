//! Gathering the turn's risky changes: changed files by path rule, the
//! turn's shell deletions, and one yes/no per ambiguous file. Each risky
//! file or deletion is suggested once; after a dismissal, never again.

use std::path::{Path, PathBuf};

use serde_json::json;
use z_engine_decisions::{AbstainReason, Answer, Question, UNCHANGED};
use z_engine_prompts::decisions::REVIEW_SUGGEST_RISKY;
use z_engine_protocol::decisions::SuggestionKind;

use super::rules::{Area, ambiguous, area_of, deletes};
use crate::decisions::context::UseContext;
use crate::decisions::suggest::{DISMISSED, offer};
use crate::decisions::uses::digest::{head, latest_request, turn_calls};
use crate::decisions::uses::guidance::{ask_each, fingerprint};

pub(super) const QUESTION: &str = "review_suggest_risky";
/// The trace name of decisions the path rules made alone.
pub(super) const RULES: &str = "review_suggest_paths";
const MAX_ASKED: usize = 4;
const MAX_PATHS: usize = 8;
const REQUEST_CHARS: usize = 300;
const TURN_CALLS: usize = 40;

pub(super) async fn suggest(cx: &UseContext, changed: &[PathBuf]) {
    let memo = cx.core.decisions.memo();
    if memo.contains(cx.feature, DISMISSED) {
        return;
    }
    let (request, calls) = cx.core.with_state(|state| {
        let working = &state.working;
        (latest_request(working), turn_calls(working, TURN_CALLS))
    });
    let fresh = |key: &str| !memo.contains(cx.feature, key);
    let paths: Vec<String> = changed
        .iter()
        .map(|path| relative(&cx.core.root, path))
        .filter(|path| fresh(path.as_str()))
        .collect();
    let deletions: Vec<String> = calls.iter().filter(|line| deletes(line)).cloned().collect();
    let deletion_key = (!deletions.is_empty()).then(|| fingerprint(json!(deletions)));
    let deletion_key = deletion_key.filter(|key| fresh(key.as_str()));
    let mut areas: Vec<Area> = Vec::new();
    let mut risky: Vec<String> = Vec::new();
    let mut unclear: Vec<String> = Vec::new();
    for path in &paths {
        match area_of(path) {
            Some(area) => {
                areas.push(area);
                risky.push(path.clone());
            }
            None if ambiguous(path) => unclear.push(path.clone()),
            None => {}
        }
    }
    if deletion_key.is_some() {
        areas.push(Area::Deletions);
    }
    let ruled = !areas.is_empty();
    unclear.truncate(MAX_ASKED);
    let judged = judge(cx, &unclear, &request).await;
    for (path, risky_file) in unclear.iter().zip(&judged) {
        if *risky_file {
            areas.push(Area::Sensitive);
            risky.push(path.clone());
        }
    }
    if areas.is_empty() {
        return;
    }
    areas.sort();
    areas.dedup();
    risky.truncate(MAX_PATHS);
    let suggestion = fingerprint(json!([risky, deletion_key]));
    if ruled {
        let answer = Answer::abstained(RULES, AbstainReason::Rules, "rules");
        cx.record(
            cx.record_of(&answer, &suggestion)
                .outcome("suggested review"),
        );
    }
    for key in risky.iter().chain(&deletion_key) {
        memo.insert(cx.feature, key);
    }
    let kind = SuggestionKind::Review {
        areas: areas.iter().map(|area| area.label().to_string()).collect(),
        paths: risky,
    };
    offer(cx, &suggestion, kind);
}

/// One yes/no per ambiguous path: is it in a risky area?
async fn judge(cx: &UseContext, paths: &[String], request: &str) -> Vec<bool> {
    if paths.is_empty() {
        return Vec::new();
    }
    let Ok(question) = Question::yes_no(QUESTION, REVIEW_SUGGEST_RISKY) else {
        return vec![false; paths.len()];
    };
    let request = head(request.trim(), REQUEST_CHARS);
    let states = paths
        .iter()
        .map(|path| json!({ "path": path, "request": request }));
    let answers = ask_each(cx, &question, states.collect()).await;
    let risky: Vec<bool> = answers
        .iter()
        .map(|(_, answer)| answer.yes() == Some(true))
        .collect();
    for ((fingerprint, answer), risky) in answers.iter().zip(&risky) {
        let outcome = if *risky {
            "suggested review"
        } else {
            UNCHANGED
        };
        cx.record(cx.record_of(answer, fingerprint).outcome(outcome));
    }
    risky
}

/// The path relative to the project root, with `/` separators.
fn relative(root: &Path, path: &Path) -> String {
    let path = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.join("/")
}
