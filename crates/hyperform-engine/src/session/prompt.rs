//! The user message that opens a turn: steering left over from an ended
//! turn, the typed text (or a command's invocation and body), attachments,
//! `@agent-` mentions, hook context, and the reminders due at a turn start
//! (interruption, plan mode, the work plan pin, finished jobs, changed
//! files).

use hyperform_context::{interrupted, plan_mode_active, render_template, wrap_reminder};
use hyperform_host::{
    DEFAULT_MAX_IMAGE_BYTES, FileKind, HostError, expand_tilde, read_image, read_pdf_base64,
    read_text, relative_display, resolve, sniff,
};
use hyperform_prompts::reminders::AGENT_MENTION;
use hyperform_protocol::{Attachment, ContentBlock, Event, MediaSource, NoticeLevel};

use crate::commands::{CommandCall, agent_mentions};
use crate::run::{RunContext, collect_reminders};
use crate::session::SessionCore;
use crate::work;

const MAX_ATTACHED_TEXT: usize = 256 * 1024;
const MAX_ATTACHED_PDF: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub(crate) struct TurnInput {
    pub text: String,
    pub attachments: Vec<Attachment>,
    /// A prompt command; its expansion replaces `text`.
    pub command: Option<CommandCall>,
}

impl TurnInput {
    pub(crate) fn new(text: String, attachments: Vec<Attachment>) -> Self {
        Self {
            text,
            attachments,
            command: None,
        }
    }

    pub(crate) fn text(text: String) -> Self {
        Self {
            text,
            ..Self::default()
        }
    }

    pub(crate) fn command(call: CommandCall) -> Self {
        Self {
            command: Some(call),
            ..Self::default()
        }
    }
}

/// The prompt text: messages still queued from an ended turn come first.
pub(crate) fn prompt_text(core: &SessionCore, input: &TurnInput) -> String {
    let queued = core.with_state(|state| std::mem::take(&mut state.queue));
    if !queued.is_empty() {
        core.events.emit(Event::QueueChanged { queued: Vec::new() });
    }
    queued
        .into_iter()
        .chain(std::iter::once(input.text.clone()))
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// One text block per non-empty entry of `texts`, then attachments and
/// reminders.
pub(crate) async fn compose(
    ctx: &RunContext,
    texts: &[String],
    attachments: &[Attachment],
    hook_context: Vec<String>,
) -> Vec<ContentBlock> {
    let mut content: Vec<ContentBlock> = texts
        .iter()
        .filter(|text| !text.is_empty())
        .map(ContentBlock::text)
        .collect();
    let known = ctx.core.agents.registry();
    for agent in agent_mentions(&texts.join("\n"), &known.names()) {
        let note = render_template(AGENT_MENTION, &[("agent", &agent)]);
        content.push(ContentBlock::text(wrap_reminder(&note)));
    }
    for attachment in attachments {
        match attach(ctx, attachment).await {
            Ok(block) => content.push(block),
            Err(error) => ctx.core.events.notice(
                NoticeLevel::Warn,
                format!("an attachment was not sent: {error}"),
            ),
        }
    }
    for context in hook_context {
        let note = format!("UserPromptSubmit hook context:\n{context}");
        content.push(ContentBlock::text(wrap_reminder(&note)));
    }
    let was_interrupted = ctx
        .core
        .with_state(|state| std::mem::take(&mut state.interrupted));
    if was_interrupted {
        content.push(ContentBlock::text(interrupted()));
    }
    if ctx.planning() {
        content.push(ContentBlock::text(plan_mode_active()));
    }
    work::at_turn_start(&ctx.core).await;
    content.extend(work::pin(&ctx.core).map(ContentBlock::text));
    content.extend(collect_reminders(ctx, &mut Vec::new()).await);
    content
}

async fn attach(ctx: &RunContext, attachment: &Attachment) -> Result<ContentBlock, HostError> {
    let path = match attachment {
        Attachment::Image { media_type, data } => {
            return Ok(ContentBlock::Image {
                source: MediaSource::Base64 {
                    media_type: media_type.clone(),
                    data: data.clone(),
                },
            });
        }
        Attachment::File { path } => path,
    };
    let root = &ctx.spec.root;
    let home = ctx.core.shared.paths.home_dir.as_deref();
    let full = resolve(root, expand_tilde(path, home));
    let shown = relative_display(root, &full);
    match sniff(&full).await? {
        FileKind::Image { .. } => {
            let (media_type, data) = read_image(&full, DEFAULT_MAX_IMAGE_BYTES).await?;
            Ok(ContentBlock::Image {
                source: MediaSource::Base64 { media_type, data },
            })
        }
        FileKind::Pdf => Ok(ContentBlock::Document {
            source: MediaSource::Base64 {
                media_type: "application/pdf".to_string(),
                data: read_pdf_base64(&full, MAX_ATTACHED_PDF).await?,
            },
            title: Some(shown),
        }),
        FileKind::Text | FileKind::Notebook => {
            let file = read_text(&full, MAX_ATTACHED_TEXT).await?;
            if !file.truncated && ctx.resources.files.record_read(&full).is_err() {
                tracing::debug!(path = %full.display(), "attached file not tracked as read");
            }
            let mut body = file.content;
            if file.truncated {
                body.push_str("\n[truncated]");
            }
            let note = format!("The user attached {shown}:\n\n{body}");
            Ok(ContentBlock::text(wrap_reminder(&note)))
        }
        FileKind::Binary => Err(HostError::Invalid(format!("{shown} is a binary file"))),
    }
}
