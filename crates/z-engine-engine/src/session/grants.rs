//! Changes to the session policy beyond approvals: rebuilding it (session
//! rules and added directories kept), a prompt command's turn-scoped
//! `allowed-tools`, and `/add-dir`.

use z_engine_config::{add_to_array, project_local_file};
use z_engine_host::{expand_tilde, resolve};
use z_engine_policy::PolicyError;
use z_engine_protocol::NoticeLevel;

use crate::error::EngineError;
use crate::session::SessionCore;
use crate::settings::{SessionSettings, build_policy};
use crate::sync::lock;

const SAVE_FLAG: &str = "--save";
pub(crate) const ADD_DIR_USAGE: &str = "Usage: /add-dir <path> [--save]";

/// Replaces the policy with one built from `settings`, keeping `rules`.
pub(crate) fn rebuild_policy(
    core: &SessionCore,
    settings: &SessionSettings,
    rules: &[String],
) -> Vec<PolicyError> {
    let home = core.shared.paths.home_dir.as_deref();
    let added = core.with_state(|state| state.added_dirs.clone());
    let (policy, errors) = build_policy(settings, &core.root, home, rules, &added);
    *lock(&core.policy) = policy;
    errors
}

/// Adds `rules` as session rules; returns the ones that were new, for
/// [`revoke_turn_rules`] when the turn ends.
pub(crate) fn grant_turn_rules(core: &SessionCore, rules: &[String]) -> Vec<String> {
    let mut ignored = Vec::new();
    let granted = {
        let mut policy = lock(&core.policy);
        let before = policy.session_rules();
        for rule in rules {
            if let Err(error) = policy.add_session_rule(rule) {
                ignored.push(format!(
                    "the allowed-tools rule `{rule}` was ignored: {error}"
                ));
            }
        }
        policy
            .session_rules()
            .into_iter()
            .filter(|rule| !before.contains(rule))
            .collect()
    };
    for text in ignored {
        core.events.notice(NoticeLevel::Warn, text);
    }
    granted
}

pub(crate) fn revoke_turn_rules(core: &SessionCore, granted: &[String]) {
    if granted.is_empty() {
        return;
    }
    let kept: Vec<String> = lock(&core.policy)
        .session_rules()
        .into_iter()
        .filter(|rule| !granted.contains(rule))
        .collect();
    for error in rebuild_policy(core, &core.settings(), &kept) {
        tracing::warn!(%error, "session rule lost while revoking turn rules");
    }
}

/// `/add-dir <path> [--save]`: the markdown answer.
pub(crate) async fn add_dir(core: &SessionCore, args: &str) -> Result<String, EngineError> {
    let args = args.trim();
    let (path, save) = match args.strip_suffix(SAVE_FLAG) {
        Some(rest) if rest.is_empty() || rest.ends_with(char::is_whitespace) => (rest.trim(), true),
        _ => (args, false),
    };
    if path.is_empty() {
        return Err(EngineError::Invalid(ADD_DIR_USAGE.to_string()));
    }
    let home = core.shared.paths.home_dir.as_deref();
    let full = resolve(&core.root, expand_tilde(path, home));
    let dir = tokio::fs::canonicalize(&full)
        .await
        .map_err(|error| EngineError::Invalid(format!("{}: {error}", full.display())))?;
    if !dir.is_dir() {
        return Err(EngineError::Invalid(format!(
            "{} is not a directory",
            dir.display()
        )));
    }
    core.with_state(|state| {
        if !state.added_dirs.contains(&dir) {
            state.added_dirs.push(dir.clone());
        }
    });
    let rules = lock(&core.policy).session_rules();
    for error in rebuild_policy(core, &core.settings(), &rules) {
        tracing::warn!(%error, "session rule lost while adding a directory");
    }
    let shown = dir.to_string_lossy().into_owned();
    let mut answer = format!("Added `{shown}` to the directories of this chat.");
    if save {
        let file = project_local_file(&core.root);
        add_to_array(&file, &["permissions", "additional_directories"], &shown)?;
        answer.push_str(&format!(" Saved to `{}`.", file.display()));
    }
    Ok(answer)
}
