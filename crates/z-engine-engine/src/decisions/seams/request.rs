//! Seam: a request is about to send the working transcript to the model
//! provider. Uses flag likely credentials in tool results not screened
//! yet; the user approves sending them through the usual approval card.
//! Declined (or unanswered) values are replaced by a placeholder in this
//! and every later request. Bypass mode only posts a notice.

use std::sync::Arc;

use serde_json::json;
use z_engine_context::render_template;
use z_engine_prompts::reminders::SECRET_MASKED;
use z_engine_protocol::{
    ApprovalDecision, ApprovalRequest, CallId, ContentBlock, Message, NoticeLevel, PermissionMode,
    Preview, RequestId, ToolResultPart,
};

use super::attention::AttentionItem;
use super::dispatch::{active, dispatch};
use crate::broker::Broker;
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::decisions::uses::secret_screen::Settled;
use crate::hooks::notify;
use crate::run::{RunContext, TranscriptSink};

const REASON: &str = "These look like credentials. Allow sends them to the model provider as they are; Deny replaces them with a placeholder in every request.";

/// The text of one tool result the provider has not seen yet.
#[derive(Debug, Clone)]
pub(crate) struct ResultText {
    pub call_id: CallId,
    pub tool: String,
    pub text: String,
}

/// A value a use believes is a credential.
#[derive(Debug, Clone)]
pub(crate) struct Finding {
    pub call_id: CallId,
    pub tool: String,
    pub value: String,
    pub kind: &'static str,
}

pub(crate) async fn screen_request(
    ctx: &RunContext,
    sink: &dyn TranscriptSink,
    working: Vec<Message>,
) -> Vec<Message> {
    screen_request_with(USES, ctx, sink, working).await
}

pub(super) async fn screen_request_with(
    uses: &[&'static dyn DecisionUse],
    ctx: &RunContext,
    sink: &dyn TranscriptSink,
    working: Vec<Message>,
) -> Vec<Message> {
    let active = active(&ctx.core, uses, Seam::BeforeRequest);
    if active.is_empty() {
        return working;
    }
    let results: Arc<[ResultText]> = unscreened(ctx, &working).into();
    if results.is_empty() {
        return working;
    }
    let agent = ctx.spec.agent_id.clone();
    let found = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let results = Arc::clone(&results);
        let cx = cx.for_agent(&agent);
        Box::pin(async move { decision_use.before_request(&cx, &results).await })
    })
    .await;
    let ledger = ctx.core.decisions.secrets();
    let (mut asked, mut withheld) = (Vec::<Finding>::new(), Vec::<Finding>::new());
    for finding in found.into_iter().flatten() {
        if asked
            .iter()
            .chain(&withheld)
            .any(|f| f.value == finding.value)
        {
            continue;
        }
        match ledger.settled(&finding.value) {
            Some(Settled::Sent) => {}
            Some(Settled::Withheld) => withheld.push(finding),
            None => asked.push(finding),
        }
    }
    let mut working = working;
    if !asked.is_empty() {
        let send = confirm(ctx, &asked).await;
        let settled = if send {
            Settled::Sent
        } else {
            Settled::Withheld
        };
        for finding in &asked {
            ledger.settle(&finding.value, settled);
        }
        if !send {
            withheld.extend(asked);
            working = sink.working();
        }
    }
    if withheld.is_empty() {
        return working;
    }
    mask(&mut working, &withheld);
    sink.set_working(working.clone());
    working
}

/// Tool results not screened before, each marked screened now.
fn unscreened(ctx: &RunContext, working: &[Message]) -> Vec<ResultText> {
    let ledger = ctx.core.decisions.secrets();
    let blocks = || working.iter().flat_map(|message| &message.content);
    let tool_of = |id: &CallId| {
        blocks().find_map(|block| match block {
            ContentBlock::ToolUse {
                id: use_id, name, ..
            } if use_id == id => Some(name.clone()),
            _ => None,
        })
    };
    blocks()
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => Some((tool_use_id, content)),
            _ => None,
        })
        .filter(|(id, _)| ledger.first_screen(id))
        .map(|(id, content)| ResultText {
            call_id: id.clone(),
            tool: tool_of(id).unwrap_or_default(),
            text: text_of(content),
        })
        .filter(|result| !result.text.is_empty())
        .collect()
}

fn text_of(content: &[ToolResultPart]) -> String {
    let texts: Vec<&str> = content
        .iter()
        .filter_map(|part| match part {
            ToolResultPart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    texts.join("\n")
}

/// True to send the values as they are. Bypass mode never asks.
async fn confirm(ctx: &RunContext, findings: &[Finding]) -> bool {
    let core = &ctx.core;
    let mut tools: Vec<&str> = findings.iter().map(|f| f.tool.as_str()).collect();
    tools.sort_unstable();
    tools.dedup();
    let count = findings.len();
    let what = if count == 1 {
        "a possible secret".to_string()
    } else {
        format!("{count} possible secrets")
    };
    let title = format!("Send {what} from {} output to the model", tools.join(", "));
    if core.mode() == PermissionMode::Bypass {
        let text = format!("{title}: Bypass mode sends them without asking.");
        core.events.notice(NoticeLevel::Warn, text);
        return true;
    }
    let lines: Vec<String> = findings
        .iter()
        .map(|f| {
            let chars = f.value.chars().count();
            format!("{} in {} output ({chars} characters)", f.kind, f.tool)
        })
        .collect();
    let request = ApprovalRequest {
        request_id: RequestId::new(),
        agent_id: ctx.spec.agent_id.clone(),
        call_id: findings[0].call_id.clone(),
        tool: findings[0].tool.clone(),
        title,
        input: json!({}),
        preview: Some(Preview::Text {
            text: lines.join("\n"),
        }),
        reason: REASON.to_string(),
        suggested_rule: None,
        can_persist: false,
    };
    let Some(reply) = core.broker.request_approvals(vec![request.clone()]).pop() else {
        return false;
    };
    let message = "Z Engine needs your permission to send possible secrets to the model";
    notify(
        core,
        message,
        vec![AttentionItem::approval(&request)],
        &ctx.cancel,
    )
    .await;
    if let Some(tracker) = &ctx.tracker {
        tracker.set_waiting(core, true);
    }
    let decision = Broker::wait(reply, &ctx.cancel).await;
    if let Some(tracker) = &ctx.tracker {
        tracker.set_waiting(core, false);
    }
    match decision {
        Some(ApprovalDecision::Deny { .. }) => false,
        Some(_) => true,
        None => {
            core.broker.withdraw_approval(&request.request_id);
            false
        }
    }
}

/// Replaces every withheld value in tool results with its placeholder.
fn mask(working: &mut [Message], withheld: &[Finding]) {
    let parts = working
        .iter_mut()
        .flat_map(|message| message.content.iter_mut())
        .filter_map(|block| match block {
            ContentBlock::ToolResult { content, .. } => Some(content),
            _ => None,
        })
        .flatten();
    for part in parts {
        let ToolResultPart::Text { text } = part else {
            continue;
        };
        for finding in withheld {
            if text.contains(&finding.value) {
                let placeholder = render_template(SECRET_MASKED, &[("kind", finding.kind)]);
                *text = text.replace(&finding.value, placeholder.trim());
            }
        }
    }
}
