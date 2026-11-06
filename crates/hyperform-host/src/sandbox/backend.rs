//! Which OS sandbox this machine offers: Seatbelt (`sandbox-exec`) on
//! macOS, bubblewrap on Linux, nothing elsewhere. Detection runs a trivial
//! sandboxed command once, so a sandbox that exists but cannot start (a
//! nested Seatbelt, user namespaces disabled) reports as unavailable.

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

pub(crate) const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxBackend {
    MacSeatbelt,
    LinuxBubblewrap,
    /// Why no sandbox can run here, for the fallback notice.
    Unavailable(String),
}

impl SandboxBackend {
    pub fn is_available(&self) -> bool {
        !matches!(self, Self::Unavailable(_))
    }

    /// A short display name (`Seatbelt`, `bubblewrap`) or the reason.
    pub fn describe(&self) -> &str {
        match self {
            Self::MacSeatbelt => "Seatbelt (sandbox-exec)",
            Self::LinuxBubblewrap => "bubblewrap",
            Self::Unavailable(reason) => reason,
        }
    }
}

/// The sandbox backend of this machine, probed on first use and cached.
/// The first call may block for a few milliseconds while the probe runs.
pub fn detect() -> SandboxBackend {
    static BACKEND: OnceLock<SandboxBackend> = OnceLock::new();
    BACKEND.get_or_init(probe).clone()
}

#[cfg(target_os = "macos")]
fn probe() -> SandboxBackend {
    let program = Path::new(SANDBOX_EXEC);
    if !program.is_file() {
        return SandboxBackend::Unavailable(format!("{SANDBOX_EXEC} is missing"));
    }
    match runs(
        program,
        &["-p", "(version 1)(allow default)", "/usr/bin/true"],
    ) {
        Ok(()) => SandboxBackend::MacSeatbelt,
        Err(reason) => SandboxBackend::Unavailable(format!("sandbox-exec cannot start: {reason}")),
    }
}

#[cfg(target_os = "linux")]
fn probe() -> SandboxBackend {
    let Some(program) = bwrap_path() else {
        return SandboxBackend::Unavailable("bubblewrap (bwrap) is not installed".to_string());
    };
    match runs(&program, &["--ro-bind", "/", "/", "true"]) {
        Ok(()) => SandboxBackend::LinuxBubblewrap,
        Err(reason) => SandboxBackend::Unavailable(format!("bwrap cannot start: {reason}")),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn probe() -> SandboxBackend {
    SandboxBackend::Unavailable("command sandboxing is not supported on this platform".to_string())
}

/// `bwrap` on `PATH`.
pub(crate) fn bwrap_path() -> Option<PathBuf> {
    which::which("bwrap").ok()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn runs(program: &Path, args: &[&str]) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(match stderr.trim() {
        "" => format!("exit status {}", output.status),
        text => text.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_is_cached_and_matches_the_platform() {
        let backend = detect();
        assert_eq!(backend, detect());
        assert!(!backend.describe().is_empty());
        #[cfg(target_os = "macos")]
        assert_ne!(backend, SandboxBackend::LinuxBubblewrap);
        #[cfg(target_os = "linux")]
        assert_ne!(backend, SandboxBackend::MacSeatbelt);
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        assert!(!backend.is_available());
    }
}
