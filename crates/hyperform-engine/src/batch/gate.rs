//! The gate each call passes in order: unknown tools, malformed JSON and
//! missing fields fail; `PreToolUse` hooks may block, rewrite the input,
//! or force a decision; the policy decides on the tool's action; a task
//! helper's write outside its task's scope turns an Allow into an Ask
//! (except in Bypass mode) and is noted in the plan; decision uses may
//! turn an Allow into an Ask (the tool-gate seam).

use std::sync::Arc;

use hyperform_llm::MalformedToolUse;
use hyperform_policy::Decision;
use hyperform_protocol::{ApprovalRequest, CallId, PermissionMode, RequestId, ToolStatus};
use hyperform_tools::Tool;
use serde_json::Value;

use super::ctx::tool_ctx;
use super::toolset::ToolSet;
use super::{schema, scope};
use crate::decisions::seams::{escalate, review_call};
use crate::hooks::{HookEvent, HookInput, PermissionOverride, run_hooks};
use crate::run::RunContext;
use crate::work;

#[derive(Debug, Clone)]
pub(crate) struct ToolCall {
    pub id: CallId,
    pub name: String,
    pub input: Value,
}

pub(super) enum Verdict {
    Run(Arc<dyn Tool>),
    /// Refused without running, with the model-facing reason.
    Refuse {
        status: ToolStatus,
        message: String,
    },
    Ask(Arc<dyn Tool>, Box<ApprovalRequest>),
}

pub(super) struct Gated {
    pub call: ToolCall,
    pub verdict: Verdict,
}

impl Gated {
    pub(super) fn refused(call: ToolCall, status: ToolStatus, message: String) -> Self {
        Self {
            call,
            verdict: Verdict::Refuse { status, message },
        }
    }
}

pub(super) async fn gate(
    ctx: &RunContext,
    tools: &ToolSet,
    call: ToolCall,
    malformed: &[MalformedToolUse],
) -> Gated {
    if let Some(bad) = malformed.iter().find(|bad| bad.id == call.id.as_str()) {
        let message = format!(
            "The input for {} is not a valid JSON object ({}). Send the call again with valid JSON.",
            call.name, bad.error
        );
        return Gated::refused(call, ToolStatus::Error, message);
    }
    let Some(tool) = tools.get(&call.name).cloned() else {
        let message = format!(
            "Unknown tool `{}`. Available tools: {}.",
            call.name,
            tools.names().join(", ")
        );
        return Gated::refused(call, ToolStatus::Error, message);
    };
    if let Err(message) = schema::check(&tool.input_schema(), &call.input) {
        return Gated::refused(call, ToolStatus::Error, message);
    }
    let mut call = call;
    let hooks = run_hooks(
        &ctx.core.hook_env(),
        &ctx.core.events,
        HookEvent::PreToolUse,
        HookInput::tool(&call.name, &call.input),
        &ctx.cancel,
    )
    .await;
    if let Some(input) = hooks.updated_input {
        call.input = input;
    }
    if let Some(reason) = hooks.blocked {
        let message = format!("A PreToolUse hook blocked this call: {reason}");
        return Gated::refused(call, ToolStatus::Denied, message);
    }
    let probe = tool_ctx(ctx, &call.id, None);
    let action = tool.action(&call.input, &probe);
    let mut decision = with_override(scope::decide(ctx, tool.name(), &action), hooks.permission);
    let agent = &ctx.spec.agent_id;
    if let Some(drift) = work::outside_scope(&ctx.core, agent, &ctx.spec.root, &action) {
        drift.record(&ctx.core, &call.name, &call.id);
        if ctx.mode() != PermissionMode::Bypass {
            decision = escalate(decision, Some(drift.reason()));
        }
    }
    let verdict = match review_call(ctx, &call, decision).await {
        Decision::Allow { .. } => Verdict::Run(tool),
        Decision::Deny { reason } => Verdict::Refuse {
            status: ToolStatus::Denied,
            message: format!("Permission denied: {reason}"),
        },
        Decision::Ask {
            reason,
            suggested_rule,
            can_persist,
        } => {
            let request = ApprovalRequest {
                request_id: RequestId::new(),
                agent_id: ctx.spec.agent_id.clone(),
                call_id: call.id.clone(),
                tool: call.name.clone(),
                title: tool.title(&call.input, &probe),
                input: call.input.clone(),
                preview: tool.preview(&call.input, &probe).await,
                reason,
                suggested_rule,
                can_persist,
            };
            Verdict::Ask(tool, Box::new(request))
        }
    };
    Gated { call, verdict }
}

/// A hook may force a decision, but never lifts a policy denial (deny
/// rules and plan mode stay authoritative).
fn with_override(
    decision: Decision,
    forced: Option<(PermissionOverride, Option<String>)>,
) -> Decision {
    let Some((forced, reason)) = forced else {
        return decision;
    };
    let hook_reason = |default: &str| reason.clone().unwrap_or_else(|| default.to_string());
    match (forced, decision) {
        (_, deny @ Decision::Deny { .. }) => deny,
        (PermissionOverride::Deny, _) => Decision::Deny {
            reason: hook_reason("a PreToolUse hook denied it"),
        },
        (PermissionOverride::Allow, _) => Decision::Allow {
            reason: hook_reason("a PreToolUse hook allowed it"),
        },
        (PermissionOverride::Ask, ask @ Decision::Ask { .. }) => ask,
        (PermissionOverride::Ask, Decision::Allow { .. }) => Decision::Ask {
            reason: hook_reason("a PreToolUse hook asks for approval"),
            suggested_rule: None,
            can_persist: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_cannot_lift_a_denial() {
        let deny = Decision::Deny {
            reason: "rule".into(),
        };
        let forced = Some((PermissionOverride::Allow, None));
        assert!(matches!(with_override(deny, forced), Decision::Deny { .. }));
        let allow = Decision::Allow {
            reason: "ok".into(),
        };
        let ask = Some((PermissionOverride::Ask, Some("check".into())));
        assert!(matches!(
            with_override(allow, ask),
            Decision::Ask {
                can_persist: false,
                ..
            }
        ));
        let ask = Decision::Ask {
            reason: "r".into(),
            suggested_rule: None,
            can_persist: true,
        };
        let forced = Some((PermissionOverride::Deny, Some("hook".into())));
        assert!(
            matches!(with_override(ask, forced), Decision::Deny { reason } if reason == "hook")
        );
    }
}
