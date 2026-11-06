//! Seam: the gate allowed a tool call (by policy or a hook). `on` uses may
//! turn that Allow into Ask; they can never allow, deny, or change an Ask.

use std::sync::Arc;

use z_engine_policy::Decision;

use super::dispatch::{active, dispatch};
use crate::batch::ToolCall;
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::run::RunContext;

pub(crate) async fn review_call(ctx: &RunContext, call: &ToolCall, decision: Decision) -> Decision {
    review_call_with(USES, ctx, call, decision).await
}

pub(super) async fn review_call_with(
    uses: &[&'static dyn DecisionUse],
    ctx: &RunContext,
    call: &ToolCall,
    decision: Decision,
) -> Decision {
    if !matches!(decision, Decision::Allow { .. }) {
        return decision;
    }
    let active = active(&ctx.core, uses, Seam::ToolGate);
    if active.is_empty() {
        return decision;
    }
    let call = Arc::new(call.clone());
    let agent = ctx.spec.agent_id.clone();
    let reasons = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let call = Arc::clone(&call);
        let cx = cx.for_agent(&agent);
        Box::pin(async move { decision_use.tool_gate(&cx, &call).await })
    })
    .await;
    escalate(decision, reasons.into_iter().flatten().next())
}

/// Like a hook's `ask` override: only an Allow becomes an Ask (offering no
/// rule to persist); Ask and Deny pass through untouched.
pub(crate) fn escalate(decision: Decision, reason: Option<String>) -> Decision {
    match (decision, reason) {
        (Decision::Allow { .. }, Some(reason)) => Decision::Ask {
            reason,
            suggested_rule: None,
            can_persist: false,
        },
        (decision, _) => decision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escalation_only_turns_allow_into_ask() {
        let allow = Decision::Allow {
            reason: "rule".into(),
        };
        assert_eq!(escalate(allow.clone(), None), allow);
        let asked = escalate(allow, Some("looks destructive".into()));
        assert_eq!(
            asked,
            Decision::Ask {
                reason: "looks destructive".into(),
                suggested_rule: None,
                can_persist: false
            }
        );
        let deny = Decision::Deny {
            reason: "deny rule".into(),
        };
        assert_eq!(escalate(deny.clone(), Some("x".into())), deny);
        let ask = Decision::Ask {
            reason: "policy".into(),
            suggested_rule: Some("Bash(ls:*)".into()),
            can_persist: true,
        };
        assert_eq!(escalate(ask.clone(), Some("x".into())), ask);
    }
}
