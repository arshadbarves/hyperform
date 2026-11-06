//! The permission policy of a session, built from its effective settings.
//! A rebuild (settings reload) keeps the rules granted during the session
//! and the directories added with `/add-dir`.

use std::path::{Path, PathBuf};

use z_engine_policy::{Policy, PolicyConfig, PolicyContext, PolicyError};

use super::effective::SessionSettings;

pub(crate) fn build_policy(
    settings: &SessionSettings,
    root: &Path,
    home: Option<&Path>,
    session_rules: &[String],
    added_dirs: &[PathBuf],
) -> (Policy, Vec<PolicyError>) {
    let permissions = &settings.settings.permissions;
    let config = PolicyConfig {
        allow: permissions.allow.clone(),
        ask: permissions.ask.clone(),
        deny: permissions.deny.clone(),
        auto_allow_read_only_bash: permissions.auto_allow_read_only_bash,
        sandbox_auto_allow: super::sandbox::auto_allow(settings),
    };
    let mut additional_dirs = settings.additional_dirs.clone();
    for dir in added_dirs {
        if !additional_dirs.contains(dir) {
            additional_dirs.push(dir.clone());
        }
    }
    let context = PolicyContext {
        project_root: root.to_path_buf(),
        additional_dirs,
        home: home.map(Path::to_path_buf),
    };
    let (mut policy, mut errors) = Policy::new(&config, context);
    for rule in session_rules {
        if let Err(error) = policy.add_session_rule(rule) {
            errors.push(error);
        }
    }
    (policy, errors)
}
