//! Wraps a shell so every command it runs starts inside the sandbox. The
//! wrapped [`ShellSpec`] keeps its [`ShellKind`](crate::ShellKind), so
//! scripts, cwd tracking, runs and background shells need no changes.

use super::backend::{SANDBOX_EXEC, SandboxBackend, bwrap_path, detect};
use super::profile::SandboxProfile;
use super::{bubblewrap, seatbelt};
use crate::HostError;
use crate::process::ShellSpec;

/// `shell` run under `backend` with `profile`. An unavailable backend is
/// [`HostError::SandboxUnavailable`] so the caller can fall back to the
/// plain shell with a notice.
pub fn wrap(
    shell: &ShellSpec,
    profile: &SandboxProfile,
    backend: &SandboxBackend,
) -> Result<ShellSpec, HostError> {
    if !shell.kind.is_posix() {
        return Err(HostError::SandboxUnavailable(format!(
            "{} is not a POSIX shell",
            shell.program.display()
        )));
    }
    let writable = profile.resolved_writable();
    let read_only = profile.resolved_read_only();
    let network = profile.allow_network;
    let (program, mut args) = match backend {
        SandboxBackend::MacSeatbelt => (
            SANDBOX_EXEC.into(),
            vec![
                "-p".to_string(),
                seatbelt::profile(&writable, &read_only, network),
            ],
        ),
        SandboxBackend::LinuxBubblewrap => {
            let program = bwrap_path().ok_or_else(|| {
                HostError::SandboxUnavailable("bubblewrap (bwrap) is not installed".to_string())
            })?;
            (program, bubblewrap::args(&writable, &read_only, network))
        }
        SandboxBackend::Unavailable(reason) => {
            return Err(HostError::SandboxUnavailable(reason.clone()));
        }
    };
    args.push(shell.program.to_string_lossy().into_owned());
    args.extend(shell.args.iter().cloned());
    Ok(ShellSpec {
        program,
        args,
        kind: shell.kind,
    })
}

/// [`wrap`] with this machine's [`detect`]ed backend.
pub fn sandbox_shell(shell: &ShellSpec, profile: &SandboxProfile) -> Result<ShellSpec, HostError> {
    wrap(shell, profile, &detect())
}

/// Output of a command whose write (or connection) the sandbox refused.
pub fn is_sandbox_denial(output: &str) -> bool {
    const MARKERS: [&str; 2] = ["Operation not permitted", "Read-only file system"];
    MARKERS.iter().any(|marker| output.contains(marker))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::process::ShellKind;

    fn zsh() -> ShellSpec {
        ShellSpec {
            program: PathBuf::from("/bin/zsh"),
            args: vec!["-c".into()],
            kind: ShellKind::Zsh,
        }
    }

    #[test]
    fn seatbelt_runs_the_shell_under_sandbox_exec() {
        let dir = tempfile::tempdir().unwrap();
        let profile = SandboxProfile::default().with_writable(dir.path());
        let spec = wrap(&zsh(), &profile, &SandboxBackend::MacSeatbelt).unwrap();
        assert_eq!(spec.program, PathBuf::from(SANDBOX_EXEC));
        assert_eq!(spec.args[0], "-p");
        let canonical = std::fs::canonicalize(dir.path()).unwrap();
        assert!(spec.args[1].contains(&*canonical.to_string_lossy()));
        assert_eq!(&spec.args[2..], ["/bin/zsh", "-c"]);
        assert_eq!(spec.kind, ShellKind::Zsh);
    }

    #[test]
    fn unavailable_backends_and_non_posix_shells_are_errors() {
        let profile = SandboxProfile::default();
        let reason = SandboxBackend::Unavailable("no sandbox here".into());
        let error = wrap(&zsh(), &profile, &reason).unwrap_err();
        assert_eq!(error.to_string(), "sandbox unavailable: no sandbox here");
        let cmd = ShellSpec {
            program: PathBuf::from("cmd.exe"),
            args: vec!["/C".into()],
            kind: ShellKind::Cmd,
        };
        let error = wrap(&cmd, &profile, &SandboxBackend::MacSeatbelt).unwrap_err();
        assert!(matches!(error, HostError::SandboxUnavailable(_)));
    }

    #[test]
    fn denial_markers_are_recognized() {
        assert!(is_sandbox_denial("touch: /x: Operation not permitted\n"));
        assert!(is_sandbox_denial(
            "mkdir: cannot create '/y': Read-only file system"
        ));
        assert!(!is_sandbox_denial("error: no such file"));
    }
}
