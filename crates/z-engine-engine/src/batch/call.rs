//! Running one approved call: `toolStarted`, throttled `toolProgress`, the
//! call itself (with a short grace period after cancellation so tools can
//! stop their processes), `PostToolUse` hooks, decision notes (the
//! after-call seam), effects, `toolFinished`.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use tokio_util::sync::CancellationToken;
use z_engine_context::wrap_reminder;
use z_engine_protocol::{ContentBlock, ToolResultPart, ToolStatus};
use z_engine_tools::{Effects, Tool, ToolError, ToolOutput, names};

use super::ctx::tool_ctx;
use super::gate::ToolCall;
use super::progress::Progress;
use super::report::{finished, started};
use crate::decisions::seams::annotate_result;
use crate::hooks::{HookEvent, HookInput, run_hooks};
use crate::run::{RunContext, reads_web};

/// How long a cancelled tool may take to stop on its own.
const CANCEL_GRACE: Duration = Duration::from_secs(3);
pub(super) const CANCELLED: &str = "cancelled by user";

pub(super) struct CallResult {
    pub block: ContentBlock,
    pub mutated: bool,
    /// Files read or written, for nested instruction discovery.
    pub touched: Vec<PathBuf>,
    pub written: Vec<PathBuf>,
}

pub(super) async fn run_call(
    ctx: &RunContext,
    tool: &Arc<dyn Tool>,
    call: &ToolCall,
) -> CallResult {
    let progress = Progress::new(
        Arc::clone(&ctx.core.events),
        ctx.spec.agent_id.clone(),
        call.id.clone(),
    );
    let tool_ctx = tool_ctx(ctx, &call.id, Some(progress.sink()));
    started(ctx, call, tool.title(&call.input, &tool_ctx));
    let clock = Instant::now();
    let result = with_grace(tool.call(call.input.clone(), &tool_ctx), &ctx.cancel).await;
    progress.flush();
    let cancelled = ctx.cancel.is_cancelled();
    if result.is_ok() && !cancelled && reads_web(tool.name()) {
        ctx.web.mark();
    }
    let (mut content, is_error, status, summary, effects) = match result {
        Ok(ToolOutput {
            mut content,
            summary,
            is_error,
            effects,
        }) => {
            let status = match (cancelled, is_error) {
                (true, _) => ToolStatus::Cancelled,
                (false, true) => ToolStatus::Error,
                (false, false) => ToolStatus::Ok,
            };
            if cancelled {
                content.push(text_part(CANCELLED));
            }
            (content, is_error || cancelled, status, summary, effects)
        }
        Err(error) => {
            let (status, text) = if cancelled || error == ToolError::Cancelled {
                (ToolStatus::Cancelled, CANCELLED.to_string())
            } else {
                (ToolStatus::Error, error.to_string())
            };
            let summary = first_line(&text);
            (
                vec![text_part(&text)],
                true,
                status,
                summary,
                Effects::default(),
            )
        }
    };
    if status != ToolStatus::Cancelled {
        content.extend(post_hooks(ctx, call, &content, is_error).await);
        annotate_result(ctx, call, status, &mut content).await;
    }
    let duration = clock.elapsed();
    finished(ctx, &call.id, status, summary, &content, duration);
    for path in &effects.files_written {
        if let Err(error) = ctx.resources.files.record_write(path) {
            tracing::debug!(path = %path.display(), %error, "written file not re-stamped");
        }
    }
    // Checks are evidence, not changes: files a check touches show up in
    // the workspace fingerprint that makes its record stale.
    let mutated = !effects.files_written.is_empty()
        || (effects.ran_command && !tool.is_read_only(&call.input) && tool.name() != names::VERIFY);
    let written = effects.files_written;
    let mut touched = effects.files_read;
    touched.extend(written.iter().cloned());
    CallResult {
        block: ContentBlock::ToolResult {
            tool_use_id: call.id.clone(),
            content,
            is_error,
        },
        mutated,
        touched,
        written,
    }
}

async fn with_grace<F>(work: F, cancel: &CancellationToken) -> Result<ToolOutput, ToolError>
where
    F: Future<Output = Result<ToolOutput, ToolError>>,
{
    tokio::pin!(work);
    tokio::select! {
        biased;
        result = &mut work => result,
        () = cancel.cancelled() => match tokio::time::timeout(CANCEL_GRACE, &mut work).await {
            Ok(result) => result,
            Err(_) => Err(ToolError::Cancelled),
        },
    }
}

/// Extra context or feedback from `PostToolUse` hooks, as result parts.
async fn post_hooks(
    ctx: &RunContext,
    call: &ToolCall,
    content: &[ToolResultPart],
    is_error: bool,
) -> Vec<ToolResultPart> {
    let text = content
        .iter()
        .filter_map(|part| match part {
            ToolResultPart::Text { text } => Some(text.as_str()),
            ToolResultPart::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let input = HookInput::tool(&call.name, &call.input).with(
        "tool_response",
        json!({ "content": text, "is_error": is_error }),
    );
    let outcome = run_hooks(
        &ctx.core.hook_env(),
        &ctx.core.events,
        HookEvent::PostToolUse,
        input,
        &ctx.cancel,
    )
    .await;
    let mut notes: Vec<String> = outcome.context;
    notes.extend(outcome.blocked);
    notes
        .into_iter()
        .map(|note| {
            text_part(&wrap_reminder(&format!(
                "PostToolUse hook feedback:\n{note}"
            )))
        })
        .collect()
}

fn text_part(text: &str) -> ToolResultPart {
    ToolResultPart::Text {
        text: text.to_string(),
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect()
}
