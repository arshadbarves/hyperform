//! Shell selection: which program runs agent commands and with which flags.
//!
//! Unix prefers a configured POSIX shell, then `$SHELL` when it is bash or
//! zsh, then `/bin/bash`, then `/bin/sh`, always as a non-login `-c` shell
//! (v1 ran `sh -c`). Windows prefers a configured shell, then Git Bash,
//! then PowerShell, then `cmd.exe` (see `shell_windows`).

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Sh,
    PowerShell,
    Cmd,
}

impl ShellKind {
    /// Bash, zsh and sh all accept POSIX `sh` syntax.
    pub fn is_posix(self) -> bool {
        matches!(self, Self::Bash | Self::Zsh | Self::Sh)
    }
}

/// How to invoke a shell: `program args... <script>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSpec {
    pub program: PathBuf,
    /// Flags placed before the command text (e.g. `-c`).
    pub args: Vec<String>,
    pub kind: ShellKind,
}

/// Resolves the shell for agent commands. `custom` is a configured shell
/// path or name; an unusable value falls back to detection.
pub fn resolve_shell(custom: Option<&str>) -> ShellSpec {
    #[cfg(windows)]
    {
        super::shell_windows::resolve(custom)
    }
    #[cfg(not(windows))]
    {
        unix::resolve(custom, std::env::var_os("SHELL"))
    }
}

#[cfg(not(windows))]
mod unix {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::{ShellKind, ShellSpec};

    pub(super) fn resolve(custom: Option<&str>, login_shell: Option<OsString>) -> ShellSpec {
        let configured = custom
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| posix_spec(Path::new(value)));
        if let Some(spec) = configured {
            return spec;
        }
        let login = login_shell
            .and_then(|shell| posix_spec(Path::new(&shell)))
            .filter(|spec| matches!(spec.kind, ShellKind::Bash | ShellKind::Zsh));
        if let Some(spec) = login {
            return spec;
        }
        ["/bin/bash", "/bin/sh"]
            .into_iter()
            .find_map(|candidate| posix_spec(Path::new(candidate)))
            .unwrap_or_else(|| spec(PathBuf::from("sh"), ShellKind::Sh))
    }

    /// A usable POSIX shell at `program` (absolute, or a name on `PATH`).
    fn posix_spec(program: &Path) -> Option<ShellSpec> {
        let kind = match program.file_name()?.to_str()? {
            "bash" => ShellKind::Bash,
            "zsh" => ShellKind::Zsh,
            "sh" | "dash" | "ksh" => ShellKind::Sh,
            _ => return None,
        };
        let program = if program.is_absolute() {
            program.to_path_buf()
        } else {
            which::which(program).ok()?
        };
        program.is_file().then(|| spec(program, kind))
    }

    fn spec(program: PathBuf, kind: ShellKind) -> ShellSpec {
        ShellSpec {
            program,
            args: vec!["-c".to_string()],
            kind,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn prefers_configured_then_login_bash_or_zsh() {
            let spec = resolve(Some("/bin/sh"), Some("/bin/zsh".into()));
            assert_eq!(spec.program, PathBuf::from("/bin/sh"));
            assert_eq!(spec.args, vec!["-c"]);

            if Path::new("/bin/zsh").is_file() {
                let spec = resolve(None, Some("/bin/zsh".into()));
                assert_eq!(spec.kind, ShellKind::Zsh);
            }

            // fish is not POSIX: fall back to bash/sh.
            let spec = resolve(Some("/usr/bin/fish"), Some("/usr/bin/fish".into()));
            assert!(matches!(spec.kind, ShellKind::Bash | ShellKind::Sh));
        }
    }
}
