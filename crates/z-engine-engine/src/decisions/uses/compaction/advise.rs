//! Relevance-aware compaction (`decisions_compaction`): when context
//! pressure clears old tool output, the model judges each older result
//! (tool, input digest, head of output) against the latest request and the
//! open todos. Unrelated results go from a quarter of the usual size floor;
//! needed ones stay. The kernel's hard keeps (files the request names, the
//! latest failing check, files edited since) never go, and with no
//! confident answer at all nothing changes.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use futures::future::join_all;
use serde_json::{Value, json};
use z_engine_config::FeatureId;
use z_engine_context::ClearTarget;
use z_engine_context::compaction::{
    KeepReason, latest_request, named_paths, plan_microcompact_ranked, protected_results,
    request_text,
};
use z_engine_context::plan_microcompact;
use z_engine_decisions::{AbstainReason, Answer, DecisionRequest, Question};
use z_engine_prompts::decisions::COMPACTION_RELEVANT;
use z_engine_protocol::{
    AgentId, CallId, ContentBlock, Message, TodoStatus, ToolResultPart, ToolStatus,
};

use super::reread::{observe, reread_key};
use crate::batch::ToolCall;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::decisions::seams::ClearAdvice;
use crate::decisions::uses::digest::{head, input_digest};
use crate::run::MIN_CLEAR_CHARS;

pub(super) const QUESTION: &str = "compaction_relevant";
/// Results asked about per pressure event, largest first.
const MAX_ASKED: usize = 16;
const REQUEST_CHARS: usize = 600;
const OUTPUT_CHARS: usize = 600;
const MAX_TODOS: usize = 5;
const TODO_CHARS: usize = 120;

#[derive(Debug)]
pub(crate) struct Compaction;

pub(crate) static COMPACTION: Compaction = Compaction;

#[async_trait]
impl DecisionUse for Compaction {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsCompaction
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::Pressure, Seam::AfterCall]
    }

    async fn pressure(
        &self,
        cx: &UseContext,
        working: &[Message],
        planned: &[ClearTarget],
    ) -> ClearAdvice {
        advise(cx, working, planned).await
    }

    async fn after_call(
        &self,
        cx: &UseContext,
        call: &ToolCall,
        _status: ToolStatus,
        _output: &[ToolResultPart],
    ) -> Vec<String> {
        observe(cx, call);
        Vec::new()
    }
}

/// One fresh answer: the result it is about and its request fingerprint.
struct Asked {
    target: ClearTarget,
    fingerprint: String,
    answer: Answer,
}

async fn advise(cx: &UseContext, working: &[Message], planned: &[ClearTarget]) -> ClearAdvice {
    let Ok(question) = Question::yes_no(QUESTION, COMPACTION_RELEVANT) else {
        return ClearAdvice::default();
    };
    let keep_recent = cx.core.settings().settings.context.keep_recent_tool_results as usize;
    let request = latest_request(working)
        .map(request_text)
        .unwrap_or_default();
    let protected = protected_results(working, &named_paths(&request));
    let revision = cx.service.revision();
    let memory = cx.core.decisions.compaction();
    let mut verdicts = memory.verdicts(&revision);
    let mut candidates: Vec<ClearTarget> =
        plan_microcompact(working, keep_recent, MIN_CLEAR_CHARS / 4)
            .into_iter()
            .filter(|target| {
                !protected.contains_key(&target.call_id) && !verdicts.contains_key(&target.call_id)
            })
            .collect();
    candidates.sort_by(|a, b| b.chars.cmp(&a.chars).then(a.message.cmp(&b.message)));
    candidates.truncate(MAX_ASKED);
    let base = base_state(cx, &request);
    let calls = calls_by_id(working);
    let asks = candidates.into_iter().map(|target| {
        let state = candidate_state(&base, &calls, working, &target);
        let (memory, revision, question) = (&memory, &revision, &question);
        async move {
            let request = DecisionRequest::new(state).ask(question.clone());
            let fingerprint = request.fingerprint();
            let answer = cx
                .ask(&request)
                .await
                .into_iter()
                .next()
                .unwrap_or_else(|| {
                    Answer::abstained(QUESTION, AbstainReason::Invalid, cx.service.provider_name())
                });
            if let Some(needed) = answer.yes() {
                memory.remember(revision, target.call_id.clone(), needed);
            }
            Asked {
                target,
                fingerprint,
                answer,
            }
        }
    });
    let asked = join_all(asks).await;
    for item in &asked {
        if let Some(needed) = item.answer.yes() {
            verdicts.insert(item.target.call_id.clone(), needed);
        }
    }
    if verdicts.is_empty() {
        for item in &asked {
            cx.record(cx.record_of(&item.answer, &item.fingerprint));
        }
        return ClearAdvice::default();
    }
    let ranked = plan_microcompact_ranked(
        working,
        keep_recent,
        MIN_CLEAR_CHARS,
        |id| verdicts.get(id).copied(),
        |id| protected.contains_key(id),
    );
    let advice = difference(planned, &ranked);
    trace(cx, &asked, &advice, planned, &protected);
    if !cx.shadow {
        for target in &advice.clear {
            if let Some((name, input)) = calls.get(&target.call_id) {
                memory.cleared(reread_key(name, input), target.call_id.clone());
            }
        }
    }
    advice
}

/// The advice turning today's `planned` clears into `ranked`.
fn difference(planned: &[ClearTarget], ranked: &[ClearTarget]) -> ClearAdvice {
    let planned_ids: HashSet<&CallId> = planned.iter().map(|target| &target.call_id).collect();
    let ranked_ids: HashSet<&CallId> = ranked.iter().map(|target| &target.call_id).collect();
    ClearAdvice {
        keep: planned
            .iter()
            .filter(|target| !ranked_ids.contains(&target.call_id))
            .map(|target| target.call_id.clone())
            .collect(),
        clear: ranked
            .iter()
            .filter(|target| !planned_ids.contains(&target.call_id))
            .cloned()
            .collect(),
    }
}

fn trace(
    cx: &UseContext,
    asked: &[Asked],
    advice: &ClearAdvice,
    planned: &[ClearTarget],
    protected: &HashMap<CallId, KeepReason>,
) {
    for item in asked {
        let id = &item.target.call_id;
        let record = cx.record_of(&item.answer, &item.fingerprint);
        let record = if advice.clear.iter().any(|target| target.call_id == *id) {
            record.outcome("cleared").saved(tokens(item.target.chars))
        } else if advice.keep.contains(id) {
            record.outcome("kept")
        } else {
            record
        };
        cx.record(record);
    }
    for target in planned {
        let Some(reason) = protected.get(&target.call_id) else {
            continue;
        };
        let kept = Answer {
            abstain: None,
            ..Answer::abstained(QUESTION, AbstainReason::Rules, "engine")
        };
        let record = cx.record_of(&kept, target.call_id.as_str());
        cx.record(record.outcome("kept").overridden(reason.label()));
    }
}

fn tokens(chars: usize) -> u64 {
    (chars as u64).div_ceil(4)
}

/// The latest request and the main agent's open todos, shared by every
/// question of one pressure event.
fn base_state(cx: &UseContext, request: &str) -> Value {
    let todos: Vec<String> = cx.core.with_state(|state| {
        let open = state
            .todos_of(&AgentId::main())
            .iter()
            .filter(|todo| todo.status != TodoStatus::Completed);
        open.take(MAX_TODOS)
            .map(|todo| head(&todo.content, TODO_CHARS))
            .collect()
    });
    json!({ "request": head(request.trim(), REQUEST_CHARS), "todos": todos })
}

fn calls_by_id(working: &[Message]) -> HashMap<CallId, (String, Value)> {
    working
        .iter()
        .flat_map(Message::tool_uses)
        .map(|(id, name, input)| (id.clone(), (name.to_string(), input.clone())))
        .collect()
}

fn candidate_state(
    base: &Value,
    calls: &HashMap<CallId, (String, Value)>,
    working: &[Message],
    target: &ClearTarget,
) -> Value {
    let (tool, input) = calls
        .get(&target.call_id)
        .map(|(name, input)| (name.as_str(), input_digest(name, input)))
        .unwrap_or(("unknown", String::new()));
    let output = match working[target.message].content.get(target.block) {
        Some(ContentBlock::ToolResult { content, .. }) => content
            .iter()
            .filter_map(|part| match part {
                ToolResultPart::Text { text } => Some(text.as_str()),
                ToolResultPart::Image { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    let mut state = base.clone();
    state["tool"] = json!(tool);
    state["input"] = json!(input);
    state["output"] = json!(head(&output, OUTPUT_CHARS));
    state
}
