//! `@path` inclusion in command bodies: paths relative to the project
//! root that the policy lets the model read, capped per file and in total,
//! inlined as fenced blocks labelled with the path.

use std::path::Path;

use z_engine_host::{FileKind, expand_tilde, read_text, relative_display, resolve, sniff};
use z_engine_policy::{Action, Decision};

use super::mentions::file_mentions;
use crate::session::SessionCore;
use crate::sync::lock;

const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_TOTAL_BYTES: usize = 1024 * 1024;

/// The included files as one section (empty when nothing was included);
/// tokens that name no file are left alone.
pub(crate) async fn included_files(core: &SessionCore, body: &str) -> String {
    let mut sections = Vec::new();
    let mut budget = MAX_TOTAL_BYTES;
    for token in file_mentions(body) {
        let home = core.shared.paths.home_dir.as_deref();
        let full = resolve(&core.root, expand_tilde(&token, home));
        let Ok(real) = tokio::fs::canonicalize(&full).await else {
            continue;
        };
        if !real.is_file() {
            continue;
        }
        let shown = relative_display(&core.root, &real);
        match include(core, &real, &shown, &mut budget).await {
            Ok(section) => sections.push(section),
            Err(reason) => sections.push(format!("[@{token} was not included: {reason}]")),
        }
    }
    sections.join("\n\n")
}

async fn include(
    core: &SessionCore,
    path: &Path,
    shown: &str,
    budget: &mut usize,
) -> Result<String, String> {
    let action = Action::Read {
        paths: vec![path.to_path_buf()],
    };
    match lock(&core.policy).decide("Read", &action, core.planning(), core.mode()) {
        Decision::Allow { .. } => {}
        Decision::Ask { reason, .. } | Decision::Deny { reason } => return Err(reason),
    }
    match sniff(path).await.map_err(|error| error.to_string())? {
        FileKind::Text | FileKind::Notebook => {}
        _ => return Err("it is not a text file".to_string()),
    }
    if *budget == 0 {
        return Err("the 1 MiB inclusion budget is used up".to_string());
    }
    let file = read_text(path, MAX_FILE_BYTES.min(*budget))
        .await
        .map_err(|error| error.to_string())?;
    *budget = budget.saturating_sub(file.content.len());
    let mut content = file.content;
    if file.truncated {
        content.push_str("\n[truncated]");
    }
    let fence = if content.contains("```") {
        "````"
    } else {
        "```"
    };
    Ok(format!("{fence}{shown}\n{}\n{fence}", content.trim_end()))
}
