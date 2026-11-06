//! `/remember <project|local|user> <text>`: appends a bullet to the chosen
//! instruction file (`AGENTS.md`, `AGENTS.local.md`, or the user's
//! `AGENTS.md`) and reloads the session's instructions.

use std::path::PathBuf;
use std::sync::Arc;

use z_engine_host::{atomic_write, read_text};

use super::reload::reload;
use crate::error::EngineError;
use crate::session::SessionCore;

const MAX_INSTRUCTIONS_BYTES: usize = 1024 * 1024;
pub(crate) const USAGE: &str = "Usage: /remember <project|local|user> <text>";

/// The file that was extended and the bullet written.
pub(crate) async fn remember(
    core: &Arc<SessionCore>,
    args: &str,
) -> Result<(PathBuf, String), EngineError> {
    let (scope, text) = args
        .trim()
        .split_once(char::is_whitespace)
        .ok_or_else(|| EngineError::Invalid(USAGE.to_string()))?;
    let path = match scope {
        "project" => core.root.join("AGENTS.md"),
        "local" => core.root.join("AGENTS.local.md"),
        "user" => core.shared.paths.config_dir.join("AGENTS.md"),
        _ => return Err(EngineError::Invalid(USAGE.to_string())),
    };
    let bullet = format!(
        "- {}",
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    );
    let existing = match read_text(&path, MAX_INSTRUCTIONS_BYTES).await {
        Ok(file) if file.truncated => {
            return Err(EngineError::Invalid(format!(
                "{} is too large to extend",
                path.display()
            )));
        }
        Ok(file) => file.content,
        Err(error) if error.is_not_found() => String::new(),
        Err(error) => return Err(error.into()),
    };
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&bullet);
    updated.push('\n');
    atomic_write(&path, updated.as_bytes()).await?;
    reload(core).await;
    Ok((path, bullet))
}
