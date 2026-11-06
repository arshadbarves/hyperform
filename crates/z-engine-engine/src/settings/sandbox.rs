//! The OS sandbox for agent shell commands and checks, derived from
//! `[shell.sandbox]`. When the sandbox is requested but the platform has
//! no backend, commands run unconfined, policy stops auto-allowing them,
//! and the settings load reports why.

use std::path::{Path, PathBuf};

use z_engine_config::Settings;
use z_engine_host::SandboxProfile;
use z_engine_host::sandbox::detect;

use super::effective::SessionSettings;

/// The profile for commands rooted at `root`; `None` when the sandbox is
/// off or unavailable. `writable_dirs` are the extra directories commands
/// may write (a worktree agent passes none of the main project's).
pub(crate) fn profile_for(
    settings: &SessionSettings,
    root: &Path,
    writable_dirs: &[PathBuf],
) -> Option<SandboxProfile> {
    let sandbox = &settings.settings.shell.sandbox;
    (sandbox.enabled && detect().is_available()).then(|| {
        SandboxProfile::for_workspace(
            root,
            writable_dirs,
            &sandbox.extra_writable,
            sandbox.allow_network,
        )
    })
}

/// Whether the policy may allow sandboxed commands without asking.
pub(crate) fn auto_allow(settings: &SessionSettings) -> bool {
    settings.settings.shell.sandbox.auto_allows() && detect().is_available()
}

/// Why a requested sandbox cannot be used on this machine.
pub(crate) fn unavailable_reason(settings: &Settings) -> Option<String> {
    if !settings.shell.sandbox.enabled {
        return None;
    }
    let backend = detect();
    (!backend.is_available()).then(|| {
        format!(
            "The command sandbox is enabled but unavailable ({}); commands run \
             unconfined and still ask for approval.",
            backend.describe()
        )
    })
}
