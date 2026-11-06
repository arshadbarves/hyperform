//! The prefetch measure, recorded when a turn ends: how many tool rounds
//! the main agent ran before its first edit, and how many files were
//! prefetched into the turn's opening message. In shadow mode it is the
//! baseline to compare `on` against.

use z_engine_decisions::{AbstainReason, Answer};
use z_engine_prompts::reminders::PREFETCHED_FILE;
use z_engine_protocol::{ContentBlock, Message, Role, TurnOutcome, TurnRecord};
use z_engine_tools::names;

use crate::decisions::context::UseContext;

pub(super) const METRIC: &str = "prefetch_first_edit";
const EDITS: [&str; 4] = [
    names::EDIT,
    names::WRITE,
    names::MULTI_EDIT,
    names::NOTEBOOK_EDIT,
];

pub(super) fn record(cx: &UseContext, turn: &TurnRecord) {
    if matches!(
        turn.outcome,
        TurnOutcome::Cancelled | TurnOutcome::Interrupted
    ) {
        return;
    }
    let (rounds, files) = cx.core.with_state(|state| measure(&state.working));
    let first_edit = match rounds {
        Some(1) => "1 tool round before the first edit".to_string(),
        Some(n) => format!("{n} tool rounds before the first edit"),
        None => "no edit".to_string(),
    };
    let outcome = format!("{first_edit}; {} prefetched", files_label(files));
    let answer = Answer::abstained(METRIC, AbstainReason::Rules, cx.service.provider_name());
    cx.record(cx.record_of(&answer, "").outcome(&outcome));
}

/// `1 file`, `3 files`.
pub(super) fn files_label(count: usize) -> String {
    match count {
        1 => "1 file".to_string(),
        n => format!("{n} files"),
    }
}

/// Tool rounds before the first edit of the latest turn (`None`: no edit),
/// and the files prefetched into its opening message.
pub(super) fn measure(working: &[Message]) -> (Option<usize>, usize) {
    let opening = working
        .iter()
        .rposition(|message| message.role == Role::User && !message.content.iter().any(is_result));
    let Some(opening) = opening else {
        return (None, 0);
    };
    let marker = prefetch_marker();
    let files = working[opening]
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.matches(marker).count()),
            _ => None,
        })
        .sum();
    let rounds = working[opening + 1..]
        .iter()
        .filter(|message| message.role == Role::Assistant)
        .map(|message| {
            message
                .tool_uses()
                .map(|(_, name, _)| name)
                .collect::<Vec<_>>()
        })
        .filter(|calls| !calls.is_empty())
        .position(|calls| calls.iter().any(|name| EDITS.contains(name)));
    (rounds, files)
}

fn is_result(block: &ContentBlock) -> bool {
    matches!(block, ContentBlock::ToolResult { .. })
}

/// The fixed text between the path and the body of a prefetched file.
fn prefetch_marker() -> &'static str {
    let after_path = PREFETCHED_FILE.split("{{path}}").nth(1).unwrap_or_default();
    after_path
        .split("{{body}}")
        .next()
        .unwrap_or_default()
        .trim()
}
