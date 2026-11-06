//! First-request context (`decisions_session_context`): before a chat's
//! first request, the model judges the project files and the deferred MCP
//! tools that share words with it. Relevant files rank right after the
//! focus files in a rebuilt repository map, which stays byte-stable from
//! that request on; relevant tools are loaded up front instead of after a
//! `LoadMcpTools` round. Later turns, and every failure, change nothing.

use std::collections::BTreeSet;

use async_trait::async_trait;
use futures::join;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_decisions::{Question, UNCHANGED};
use z_engine_prompts::decisions::{SESSION_CONTEXT_FILE, SESSION_CONTEXT_TOOL};

use super::candidates::{self, outline};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::decisions::uses::digest::head;
use crate::decisions::uses::guidance::{ask_each, content_words};
use crate::run::repo_map_files;

pub(super) const FILE_QUESTION: &str = "session_context_file";
pub(super) const TOOL_QUESTION: &str = "session_context_tool";
const REQUEST_CHARS: usize = 600;
const DESCRIPTION_CHARS: usize = 300;

#[derive(Debug)]
pub(crate) struct SessionContext;

pub(crate) static SESSION_CONTEXT: SessionContext = SessionContext;

#[async_trait]
impl DecisionUse for SessionContext {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsSessionContext
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::TurnStart]
    }

    async fn turn_start(&self, cx: &UseContext, text: &str) -> Vec<String> {
        if !cx.core.with_state(|state| state.turns.is_empty()) {
            return Vec::new();
        }
        let words = content_words(text);
        let request = head(text.trim(), REQUEST_CHARS);
        let (files, tools) = join!(
            relevant_files(cx, &words, &request),
            relevant_tools(cx, &words, &request),
        );
        if !cx.shadow {
            cx.core.repo_map.rank_first(files);
            cx.core.decisions.preload().set(tools);
        }
        Vec::new()
    }
}

async fn relevant_files(cx: &UseContext, words: &BTreeSet<String>, request: &str) -> Vec<String> {
    let paths = repo_map_files(&cx.core).await;
    let map = cx.core.repo_map.peek().unwrap_or_default();
    let asked = candidates::files(words, &paths, &map);
    let Ok(question) = Question::yes_no(FILE_QUESTION, SESSION_CONTEXT_FILE) else {
        return Vec::new();
    };
    let states = asked
        .iter()
        .map(|path| json!({ "request": request, "file": path, "outline": outline(&map, path) }));
    let answers = ask_each(cx, &question, states.collect()).await;
    picked(cx, asked, answers, "ranked first")
}

async fn relevant_tools(cx: &UseContext, words: &BTreeSet<String>, request: &str) -> Vec<String> {
    let catalog = cx.core.mcp.catalog();
    if !catalog.deferred() {
        return Vec::new();
    }
    let all = catalog.tools.iter().map(|tool| {
        let description = tool.info.description.as_deref().unwrap_or_default();
        (tool.name.clone(), head(description, DESCRIPTION_CHARS))
    });
    let asked = candidates::tools(words, all.collect());
    let Ok(question) = Question::yes_no(TOOL_QUESTION, SESSION_CONTEXT_TOOL) else {
        return Vec::new();
    };
    let states = asked.iter().map(|(name, description)| {
        json!({ "request": request, "tool": name, "description": description })
    });
    let answers = ask_each(cx, &question, states.collect()).await;
    let names = asked.into_iter().map(|(name, _)| name).collect();
    picked(cx, names, answers, "loaded up front")
}

/// The items the model confidently called relevant; one trace record each.
fn picked(
    cx: &UseContext,
    items: Vec<String>,
    answers: Vec<(String, z_engine_decisions::Answer)>,
    outcome: &str,
) -> Vec<String> {
    let mut relevant = Vec::new();
    for (item, (fingerprint, answer)) in items.into_iter().zip(answers) {
        let yes = answer.yes() == Some(true);
        cx.record(cx.record_of(&answer, &fingerprint).outcome(if yes {
            outcome
        } else {
            UNCHANGED
        }));
        if yes {
            relevant.push(item);
        }
    }
    relevant
}
