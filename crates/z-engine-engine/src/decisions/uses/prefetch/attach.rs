//! Reading prefetched files the way user attachments are read: only text
//! files inside the project that the policy lets the agent Read without
//! asking, not read yet, and within the `[decisions.prefetch]` budget.

use std::path::{Path, PathBuf};

use z_engine_context::{estimate_text, render_template};
use z_engine_host::{FileKind, read_text, sniff};
use z_engine_policy::{Action, Decision};
use z_engine_prompts::reminders::PREFETCHED_FILE;

use super::candidates::Candidate;
use crate::session::SessionCore;
use crate::sync::lock;

/// Larger files are never prefetched whole.
const MAX_FILE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub(super) struct Attached {
    pub full: PathBuf,
    pub tokens: u64,
    /// The reminder text for the opening message.
    pub note: String,
}

/// The candidates the agent could read right now without asking.
pub(super) async fn eligible(core: &SessionCore, candidates: Vec<Candidate>) -> Vec<Candidate> {
    let mut kept = Vec::new();
    for candidate in candidates {
        let full = core.root.join(&candidate.path);
        if full.is_file()
            && full.starts_with(&core.root)
            && !core.main.files.was_read(&full)
            && readable(core, &full)
            && matches!(sniff(&full).await, Ok(FileKind::Text))
        {
            kept.push(candidate);
        }
    }
    kept
}

/// Allowed by the policy in the session's mode: Ask and Deny never prefetch.
fn readable(core: &SessionCore, full: &Path) -> bool {
    let action = Action::Read {
        paths: vec![full.to_path_buf()],
    };
    let decision = lock(&core.policy).decide("Read", &action, core.planning(), core.mode());
    matches!(decision, Decision::Allow { .. })
}

/// `picks` in order while at most `max_files` fit in `max_tokens`; a file
/// that does not fit is skipped, a later smaller one may still fit.
pub(super) async fn read_within(
    core: &SessionCore,
    picks: &[Candidate],
    max_files: usize,
    max_tokens: u64,
) -> Vec<Attached> {
    let mut attached: Vec<Attached> = Vec::new();
    let mut used = 0;
    for pick in picks {
        if attached.len() == max_files {
            break;
        }
        let full = core.root.join(&pick.path);
        let Ok(file) = read_text(&full, MAX_FILE_BYTES).await else {
            continue;
        };
        let tokens = estimate_text(&file.content);
        if file.truncated || used + tokens > max_tokens {
            continue;
        }
        used += tokens;
        let values = [
            ("path", pick.path.as_str()),
            ("body", file.content.as_str()),
        ];
        attached.push(Attached {
            full,
            tokens,
            note: render_template(PREFETCHED_FILE, &values),
        });
    }
    attached
}
