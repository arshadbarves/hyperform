//! One tool batch: gate every call in order, request the approvals
//! together, then execute in call order. Neighbouring concurrency-safe
//! calls run together; any other call is a barrier and runs alone. Every
//! call ends with exactly one result, in call order.

use std::path::PathBuf;

use futures::future::join_all;
use z_engine_context::nested_instructions;
use z_engine_llm::MalformedToolUse;
use z_engine_protocol::{ContentBlock, ToolStatus, now_ms};
use z_engine_tools::names;

use super::approval;
use super::call::{CANCELLED, CallResult, run_call};
use super::diagnose::append_lsp_errors;
use super::gate::{Gated, ToolCall, Verdict, gate};
use super::report::refused;
use super::toolset::ToolSet;
use crate::run::RunContext;
use crate::settings::{nested_docs, scoped_rules};
use crate::sync::lock;

#[derive(Debug, Default)]
pub(crate) struct BatchOutcome {
    /// One `tool_result` per call, in call order.
    pub results: Vec<ContentBlock>,
    pub cancelled: bool,
    pub mutated: bool,
    /// Epoch ms right after the last call that changed something.
    pub mutated_at: Option<u64>,
    pub used_todo_write: bool,
    /// Files the calls wrote.
    pub written: Vec<PathBuf>,
}

pub(crate) async fn run_batch(
    ctx: &RunContext,
    tools: &ToolSet,
    calls: Vec<ToolCall>,
    malformed: &[MalformedToolUse],
    pending: &mut Vec<String>,
) -> BatchOutcome {
    let used_todo_write = calls.iter().any(|call| call.name == names::TODO_WRITE);
    let mut gated = Vec::with_capacity(calls.len());
    for call in calls {
        if ctx.cancel.is_cancelled() {
            gated.push(Gated::refused(
                call,
                ToolStatus::Cancelled,
                CANCELLED.into(),
            ));
        } else {
            gated.push(gate(ctx, tools, call, malformed).await);
        }
    }
    approval::resolve(ctx, &mut gated).await;
    let mut outcome = execute(ctx, gated, pending).await;
    outcome.used_todo_write = used_todo_write;
    outcome
}

async fn execute(ctx: &RunContext, gated: Vec<Gated>, pending: &mut Vec<String>) -> BatchOutcome {
    let mut results: Vec<Option<ContentBlock>> = gated.iter().map(|_| None).collect();
    let mut mutated_at = None;
    let mut touched: Vec<PathBuf> = Vec::new();
    let mut written: Vec<(usize, Vec<PathBuf>)> = Vec::new();
    let mut index = 0;
    while index < gated.len() {
        if ctx.cancel.is_cancelled() {
            break;
        }
        let mut segment = Vec::new();
        let mut next = index;
        while next < gated.len() {
            let entry = &gated[next];
            match &entry.verdict {
                Verdict::Refuse { status, message } => {
                    results[next] = Some(refused(ctx, &entry.call, *status, message));
                }
                Verdict::Ask(..) => {
                    let block = refused(ctx, &entry.call, ToolStatus::Cancelled, CANCELLED);
                    results[next] = Some(block);
                }
                Verdict::Run(tool) if tool.is_concurrency_safe(&entry.call.input) => {
                    segment.push(next);
                }
                Verdict::Run(_) if segment.is_empty() => {
                    segment.push(next);
                    next += 1;
                    break;
                }
                Verdict::Run(_) => break,
            }
            next += 1;
        }
        let calls = &gated;
        let runs = segment.iter().filter_map(|&at| match &calls[at].verdict {
            Verdict::Run(tool) => {
                Some(async move { (at, run_call(ctx, tool, &calls[at].call).await) })
            }
            _ => None,
        });
        for (
            at,
            CallResult {
                block,
                mutated: changed,
                touched: files,
                written: wrote,
            },
        ) in join_all(runs).await
        {
            results[at] = Some(block);
            if changed {
                mutated_at = Some(now_ms());
            }
            touched.extend(files);
            if !wrote.is_empty() {
                written.push((at, wrote));
            }
        }
        index = next;
    }
    if !touched.is_empty() {
        announce_nested(ctx, &touched, pending);
    }
    if !written.is_empty() && !ctx.cancel.is_cancelled() {
        append_lsp_errors(ctx, &mut results, &written).await;
    }
    let cancelled = ctx.cancel.is_cancelled();
    let results = results
        .into_iter()
        .zip(&gated)
        .map(|(result, entry)| {
            result.unwrap_or_else(|| refused(ctx, &entry.call, ToolStatus::Cancelled, CANCELLED))
        })
        .collect();
    BatchOutcome {
        results,
        cancelled,
        mutated: mutated_at.is_some(),
        mutated_at,
        used_todo_write: false,
        written: written.into_iter().flat_map(|(_, files)| files).collect(),
    }
}

/// Queues instruction files from directories the calls touched, and rules
/// whose globs match the touched files, that this agent has not seen yet.
fn announce_nested(ctx: &RunContext, touched: &[PathBuf], pending: &mut Vec<String>) {
    let settings = ctx.core.settings();
    let compat = settings.settings.compat.claude;
    let docs = {
        let mut seen = lock(&ctx.resources.seen_instructions);
        let files = || touched.iter().map(PathBuf::as_path);
        let mut docs = nested_docs(&ctx.spec.root, files(), compat, &mut seen);
        let rules = &settings.extensions.rules;
        docs.extend(scoped_rules(&ctx.spec.root, files(), rules, &mut seen));
        docs
    };
    if !docs.is_empty() {
        pending.push(nested_instructions(&docs));
    }
}
