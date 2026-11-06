//! The plan audit by the read-only `audit` agent: one model call with the
//! plan as JSON, answered with JSON findings that are recorded as model
//! reviews. It runs for deep plans when planning ends (`ExitPlanMode`) and
//! when the user asks (`requestAudit`). Any failure records nothing and
//! leaves a quiet notice.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use z_engine_context::audit_request;
use z_engine_llm::{ModelRequest, SystemBlock};
use z_engine_protocol::work::{
    Actor, AuditFinding, AuditKind, AuditRef, AuditSeverity, AuditSource, AuditStatus, EntryStatus,
    PlanDepth, TaskStatus, WorkCause,
};
use z_engine_protocol::{AgentId, Message, NoticeLevel};
use z_engine_tools::names;
use z_engine_work::{PlanState, Proposal, Submission, graph};

use super::facts::instant;
use super::lifecycle::{attached, reassess_depth, report};
use crate::orchestration::model_for;
use crate::run::side_request;
use crate::session::SessionCore;

/// The built-in agent definition the audit runs with (custom ones of the
/// same name override it).
pub(crate) const AUDIT_AGENT: &str = "audit";
const AUDIT_MAX_TOKENS: u32 = 4_096;
const MAX_FINDINGS: usize = 20;
const MAX_ENTRIES: usize = 40;
const DOCUMENT_CHARS: usize = 8_000;

/// `ExitPlanMode`: the plan's depth is assessed again from all its tasks,
/// and on a deep plan the audit runs while the user reviews.
pub(crate) fn review_deep_plan(core: &Arc<SessionCore>) {
    let Some(plan) = attached(core) else {
        return;
    };
    let cause = WorkCause::Tool {
        tool: names::EXIT_PLAN_MODE.to_string(),
        call_id: None,
    };
    reassess_depth(core, &plan, cause);
    if plan.read(|s| s.header.depth) == PlanDepth::Deep {
        request(core, false);
    }
}

/// Runs the audit in the background; `asked` by the user (a notice says
/// when it found nothing).
pub(crate) fn request(core: &Arc<SessionCore>, asked: bool) {
    let core = Arc::clone(core);
    tokio::spawn(async move {
        match review(&core).await {
            Ok(0) if asked => {
                core.events
                    .notice(NoticeLevel::Info, "The plan audit found nothing to add.");
            }
            Ok(_) => {}
            Err(error) => core.events.notice(
                NoticeLevel::Info,
                format!("The plan audit did not finish: {error}"),
            ),
        }
    });
}

/// The number of findings recorded.
async fn review(core: &Arc<SessionCore>) -> Result<usize, String> {
    let plan = attached(core).ok_or("no work plan is attached")?;
    let registry = core.agents.registry();
    let def = registry
        .get(AUDIT_AGENT)
        .ok_or("the audit agent is not available")?;
    let model = model_for(def, &core.settings().settings, &core.main_model(), None);
    let snapshot = plan.read(plan_json);
    let request = ModelRequest::new(model, vec![Message::user_text(audit_request(&snapshot))])
        .with_system(vec![SystemBlock::new(def.prompt.clone())])
        .with_max_tokens(AUDIT_MAX_TOKENS);
    let cancel = core.cancel.child_token();
    let turn = side_request(core, &AgentId::main(), request, &cancel)
        .await
        .map_err(|error| error.to_string())?;
    let found = plan.read(|state| parse(state, &turn.text()))?;
    let count = found.len();
    if count == 0 {
        return Ok(0);
    }
    let result = plan.submit(&instant(core), |_| {
        let proposals = vec![Proposal::Audit {
            source: AuditSource::AuditAgent,
            findings: found,
        }];
        Ok(Submission::new(Actor::Kernel, WorkCause::Edit, proposals))
    });
    report(core, result, "The plan audit was not recorded");
    Ok(count)
}

/// What the audit agent reads.
fn plan_json(state: &PlanState) -> String {
    let header = &state.header;
    let items: Vec<Value> = state
        .items()
        .map(|item| json!({"id": item.id, "kind": item.kind, "text": item.text, "status": item.status}))
        .collect();
    let tasks: Vec<Value> = graph::tree_order(state)
        .iter()
        .filter_map(|id| state.task(id))
        .filter(|task| {
            !matches!(
                task.status,
                TaskStatus::Cancelled | TaskStatus::Superseded { .. }
            )
        })
        .map(|task| {
            json!({
                "id": task.id, "parent": task.parent, "title": task.title,
                "description": task.description, "kind": task.kind, "status": task.status,
                "dependsOn": task.depends_on, "requirements": task.requirements,
                "scope": task.scope, "acceptance": task.acceptance.iter().map(|criterion| json!({
                    "id": criterion.id, "text": criterion.text,
                    "method": criterion.method, "status": criterion.status,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let entries: Vec<Value> = state
        .entries
        .values()
        .filter(|entry| matches!(entry.status, EntryStatus::Accepted | EntryStatus::Proposed))
        .rev()
        .take(MAX_ENTRIES)
        .map(|entry| json!({"id": entry.id, "entry": entry.body, "tasks": entry.tasks}))
        .collect();
    let findings: Vec<Value> = state
        .audit
        .iter()
        .filter(|finding| matches!(finding.status, AuditStatus::Open | AuditStatus::Added))
        .map(|finding| json!({"kind": finding.kind, "text": finding.text}))
        .collect();
    let document: String = header.document.chars().take(DOCUMENT_CHARS).collect();
    let plan = json!({
        "goal": header.goal, "depth": header.depth, "document": document,
        "items": items, "tasks": tasks, "entries": entries, "findings": findings,
    });
    serde_json::to_string_pretty(&plan).unwrap_or_default()
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    findings: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    #[serde(default)]
    severity: Option<AuditSeverity>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    suggestion: Option<String>,
    #[serde(default)]
    refs: Vec<Value>,
}

/// The findings of an answer; refs the plan does not have are dropped,
/// and a finding the agent already made keeps its id.
pub(super) fn parse(state: &PlanState, text: &str) -> Result<Vec<AuditFinding>, String> {
    let (start, end) = (text.find('{'), text.rfind('}'));
    let body = match (start, end) {
        (Some(start), Some(end)) if start < end => &text[start..=end],
        _ => return Err("the audit agent did not answer with JSON".into()),
    };
    let answer: Answer = serde_json::from_str(body)
        .map_err(|error| format!("the audit agent's answer could not be read: {error}"))?;
    let findings = answer
        .findings
        .into_iter()
        .filter(|item| !item.text.trim().is_empty())
        .take(MAX_FINDINGS)
        .map(|item| {
            let text = item.text.trim().to_string();
            let id = state
                .audit
                .iter()
                .find(|old| old.source == AuditSource::AuditAgent && same(&old.text, &text))
                .map(|old| old.id.clone())
                .unwrap_or_default();
            AuditFinding {
                id,
                kind: AuditKind::ModelReview,
                severity: item.severity.unwrap_or(AuditSeverity::Medium),
                text,
                refs: item
                    .refs
                    .into_iter()
                    .filter_map(|raw| known(state, raw))
                    .collect(),
                suggestion: item
                    .suggestion
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
                status: AuditStatus::Open,
                source: AuditSource::AuditAgent,
                created_revision: 0,
                revision: 0,
            }
        })
        .collect();
    Ok(findings)
}

fn known(state: &PlanState, raw: Value) -> Option<AuditRef> {
    let reference: AuditRef = serde_json::from_value(raw).ok()?;
    let exists = match &reference {
        AuditRef::Task { task_id } => state.task(task_id).is_some(),
        AuditRef::Criterion {
            task_id,
            criterion_id,
        } => state
            .task(task_id)
            .is_some_and(|task| task.acceptance.iter().any(|c| c.id == *criterion_id)),
        AuditRef::Item { item_id } => state.item(item_id).is_some(),
        AuditRef::File { path } => !path.trim().is_empty(),
    };
    exists.then_some(reference)
}

fn same(a: &str, b: &str) -> bool {
    let words = |text: &str| {
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    words(a) == words(b)
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
