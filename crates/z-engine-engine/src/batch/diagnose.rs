//! After a batch wrote files that a language server covers, error
//! diagnostics are appended to the result of the call that wrote them.

use std::path::PathBuf;

use z_engine_protocol::{ContentBlock, ToolResultPart};

use crate::lsp::error_notes;
use crate::run::RunContext;

/// `written`: per call index, the files that call wrote.
pub(super) async fn append_lsp_errors(
    ctx: &RunContext,
    results: &mut [Option<ContentBlock>],
    written: &[(usize, Vec<PathBuf>)],
) {
    let lsp = &ctx.core.lsp;
    let Some(worker) = lsp.worker() else {
        return;
    };
    let mut files: Vec<PathBuf> = written
        .iter()
        .flat_map(|(_, files)| files.iter())
        .filter(|file| lsp.covers(file))
        .cloned()
        .collect();
    files.sort();
    files.dedup();
    if files.is_empty() {
        return;
    }
    let notes = error_notes(worker, &ctx.spec.root, files).await;
    if notes.is_empty() {
        return;
    }
    for (index, files) in written {
        let Some(Some(ContentBlock::ToolResult { content, .. })) = results.get_mut(*index) else {
            continue;
        };
        for file in files {
            if let Some(note) = notes.get(file) {
                content.push(ToolResultPart::Text { text: note.clone() });
            }
        }
    }
}
