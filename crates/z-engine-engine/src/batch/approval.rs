//! Every approval of a batch is requested at once and the user may answer
//! in any order. Rules granted by "allow for the session/project" are
//! applied by the session actor when the answer arrives.

use futures::future::join_all;
use z_engine_protocol::{ApprovalDecision, ApprovalRequest, ToolStatus};

use super::gate::{Gated, Verdict};
use crate::broker::Broker;
use crate::decisions::seams::AttentionItem;
use crate::hooks::notify;
use crate::run::RunContext;

pub(super) async fn resolve(ctx: &RunContext, gated: &mut [Gated]) {
    let asks: Vec<(usize, ApprovalRequest)> = gated
        .iter()
        .enumerate()
        .filter_map(|(index, call)| match &call.verdict {
            Verdict::Ask(_, request) => Some((index, (**request).clone())),
            _ => None,
        })
        .collect();
    if asks.is_empty() {
        return;
    }
    let broker = &ctx.core.broker;
    let requests = asks.iter().map(|(_, request)| request.clone()).collect();
    let replies = broker.request_approvals(requests);
    let tools: Vec<&str> = asks
        .iter()
        .map(|(_, request)| request.tool.as_str())
        .collect();
    let message = format!("Z Engine needs your permission to use {}", tools.join(", "));
    let items = asks
        .iter()
        .map(|(_, request)| AttentionItem::approval(request))
        .collect();
    notify(&ctx.core, &message, items, &ctx.cancel).await;
    let waits = replies
        .into_iter()
        .map(|reply| Broker::wait(reply, &ctx.cancel));
    if let Some(tracker) = &ctx.tracker {
        tracker.set_waiting(&ctx.core, true);
    }
    let decisions = join_all(waits).await;
    if let Some(tracker) = &ctx.tracker {
        tracker.set_waiting(&ctx.core, false);
    }
    for ((index, request), decision) in asks.into_iter().zip(decisions) {
        let verdict = std::mem::replace(
            &mut gated[index].verdict,
            Verdict::Refuse {
                status: ToolStatus::Cancelled,
                message: "cancelled by user".to_string(),
            },
        );
        let Verdict::Ask(tool, _) = verdict else {
            continue;
        };
        gated[index].verdict = match decision {
            Some(ApprovalDecision::Deny { feedback }) => Verdict::Refuse {
                status: ToolStatus::Denied,
                message: denial(feedback.as_deref()),
            },
            Some(_) => Verdict::Run(tool),
            None => {
                broker.withdraw_approval(&request.request_id);
                Verdict::Refuse {
                    status: ToolStatus::Cancelled,
                    message: "cancelled by user".to_string(),
                }
            }
        };
    }
}

fn denial(feedback: Option<&str>) -> String {
    match feedback.map(str::trim).filter(|text| !text.is_empty()) {
        Some(feedback) => {
            format!("The user denied this action.\nFeedback from the user: {feedback}")
        }
        None => "The user denied this action.".to_string(),
    }
}
