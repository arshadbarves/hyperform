//! Persistent working directory (port of v1 `bash_script`): wraps a command
//! so the shell records its final directory, letting `cd` carry over to the
//! next call. v1 printed a marker on stderr; with live output streaming the
//! marker would leak into the stream, so the probe writes to a private temp
//! file instead.

use std::path::{Path, PathBuf};

use super::shell::ShellKind;

/// A pending report file; removed when dropped.
#[derive(Debug)]
pub(crate) struct CwdProbe {
    file: PathBuf,
}

impl CwdProbe {
    pub(crate) fn new() -> Self {
        Self {
            file: std::env::temp_dir().join(format!("zengine-cwd-{}", ulid::Ulid::new())),
        }
    }

    /// `command` followed by the probe; the command's exit status is kept.
    pub(crate) fn wrap(&self, kind: ShellKind, command: &str) -> String {
        let file = self.file.to_string_lossy();
        match kind {
            ShellKind::Bash | ShellKind::Zsh | ShellKind::Sh => format!(
                // `status` is read-only in zsh, hence the prefixed name.
                "{command}\n__zengine_status=$?\nprintf '%s' \"$PWD\" >| {} 2>/dev/null\nexit $__zengine_status\n",
                sh_quote(&file)
            ),
            ShellKind::PowerShell => format!(
                "{command}\n$__zengine_status = $LASTEXITCODE\nif ($null -eq $__zengine_status) {{ $__zengine_status = 0 }}\n[System.IO.File]::WriteAllText('{}', (Get-Location).Path)\nexit $__zengine_status",
                ps_quote(&file)
            ),
            ShellKind::Cmd => format!(
                "{command}\r\nset ZENGINE_STATUS=%ERRORLEVEL%\r\ncd > {}\r\nexit /b %ZENGINE_STATUS%\r\n",
                cmd_quote(&file)
            ),
        }
    }

    /// The directory the shell ended in, when the probe ran.
    pub(crate) fn read(&self) -> Option<PathBuf> {
        let text = std::fs::read_to_string(&self.file).ok()?;
        let dir = text.trim_end_matches(['\r', '\n']);
        (!dir.is_empty()).then(|| PathBuf::from(dir))
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.file
    }
}

impl Drop for CwdProbe {
    fn drop(&mut self) {
        remove_quietly(&self.file);
    }
}

fn remove_quietly(path: &Path) {
    if let Err(e) = std::fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::debug!(path = %path.display(), error = %e, "could not remove cwd probe");
        }
    }
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

fn cmd_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_matches_each_shell() {
        assert_eq!(sh_quote("/tmp/a b"), "'/tmp/a b'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
        assert_eq!(ps_quote("it's"), "it''s");
        assert_eq!(cmd_quote(r#"say "hi""#), r#""say ""hi""""#);
    }

    #[test]
    fn scripts_keep_the_exit_status_and_report_the_directory() {
        let probe = CwdProbe::new();
        let posix = probe.wrap(ShellKind::Zsh, "cd /tmp");
        assert!(posix.starts_with("cd /tmp\n__zengine_status=$?"));
        assert!(posix.contains("\"$PWD\""));
        assert!(posix.ends_with("exit $__zengine_status\n"));
        let ps = probe.wrap(ShellKind::PowerShell, "cargo test");
        assert!(ps.contains("(Get-Location).Path"));
        assert!(ps.contains("$__zengine_status = 0"));
        assert!(
            probe
                .wrap(ShellKind::Cmd, "dir")
                .contains("exit /b %ZENGINE_STATUS%")
        );
    }

    #[test]
    fn read_trims_line_endings_and_drop_removes_the_file() {
        let probe = CwdProbe::new();
        assert_eq!(probe.read(), None);
        std::fs::write(probe.path(), "C:\\work\\dir\r\n").unwrap();
        assert_eq!(probe.read(), Some(PathBuf::from("C:\\work\\dir")));
        let file = probe.path().to_path_buf();
        drop(probe);
        assert!(!file.exists());
    }
}
