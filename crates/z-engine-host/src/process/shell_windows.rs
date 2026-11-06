//! Windows shell detection (v1 `shell_detect`, OpenCode's pattern).
//!
//! Git Bash is found through the Git install location, never through
//! `PATH`, so WSL's `C:\Windows\System32\bash.exe` shim is never selected.
//! Every candidate is validated with a hidden window before use.

use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};

use super::kill::CREATE_NO_WINDOW;
use super::shell::{ShellKind, ShellSpec};

pub(super) fn resolve(custom: Option<&str>) -> ShellSpec {
    if let Some(spec) = custom.and_then(resolve_override) {
        return spec;
    }
    if let Some(path) = git_bash_path().filter(|p| is_bash_usable(p)) {
        tracing::info!(?path, "using Git Bash as the agent shell");
        return bash(path);
    }
    if let Some(path) = find_powershell() {
        tracing::info!(?path, "using PowerShell as the agent shell");
        return powershell(path);
    }
    tracing::info!("using cmd.exe as the agent shell");
    cmd()
}

fn bash(program: PathBuf) -> ShellSpec {
    ShellSpec {
        program,
        args: vec!["-lc".to_string()],
        kind: ShellKind::Bash,
    }
}

fn powershell(program: PathBuf) -> ShellSpec {
    ShellSpec {
        program,
        args: vec!["-NoProfile".to_string(), "-Command".to_string()],
        kind: ShellKind::PowerShell,
    }
}

fn cmd() -> ShellSpec {
    ShellSpec {
        program: PathBuf::from("cmd.exe"),
        args: vec!["/C".to_string()],
        kind: ShellKind::Cmd,
    }
}

fn resolve_override(value: &str) -> Option<ShellSpec> {
    let trimmed = value.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "" => None,
        "powershell" | "pwsh" => find_powershell().map(powershell),
        "cmd" => Some(cmd()),
        "bash" | "git-bash" | "gitbash" => git_bash_path().filter(|p| is_bash_usable(p)).map(bash),
        _ => {
            let path = PathBuf::from(trimmed);
            let stem = path.file_stem()?.to_str()?.to_ascii_lowercase();
            match stem.as_str() {
                "bash" if !is_wsl_shim(&path) && path.is_file() && is_bash_usable(&path) => {
                    Some(bash(path))
                }
                "pwsh" | "powershell" if is_powershell_usable(&path) => Some(powershell(path)),
                "cmd" => Some(cmd()),
                _ => {
                    tracing::warn!(shell = trimmed, "configured shell is not usable; detecting");
                    None
                }
            }
        }
    }
}

/// `C:\Windows\System32\bash.exe` is a WSL stub that fails without WSL.
fn is_wsl_shim(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|dir| dir.to_str())
        .is_some_and(|dir| dir.eq_ignore_ascii_case("system32"))
}

fn git_bash_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("HYPERFORM_GIT_BASH_PATH") {
        let path = PathBuf::from(explicit.trim());
        if path.is_file() {
            return Some(path);
        }
    }
    let git = which::which("git").ok()?;
    let root = git.parent()?.parent()?;
    [
        root.join("bin").join("bash.exe"),
        root.join("usr").join("bin").join("bash.exe"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

fn find_powershell() -> Option<PathBuf> {
    let on_path = ["pwsh", "powershell"]
        .into_iter()
        .filter_map(|name| which::which(name).ok());
    let fallbacks = [
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
    ]
    .into_iter()
    .map(PathBuf::from)
    .filter(|path| path.exists());
    on_path
        .chain(fallbacks)
        .find(|path| is_powershell_usable(path))
}

fn is_powershell_usable(path: &Path) -> bool {
    probe(path, &["-NoProfile", "-Command", "Write-Output ok"])
}

/// WSL's shim fails `--version` when WSL is off, so this doubles as a guard.
fn is_bash_usable(path: &Path) -> bool {
    probe(path, &["--version"])
}

fn probe(program: &Path, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|status| status.success())
}
