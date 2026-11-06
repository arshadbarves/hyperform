//! ripgrep backend: one `rg` process with NUL-separated paths and line
//! numbers always on, parsed into the shared hit model.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::sync::CancellationToken;

use super::format::{Collected, FileMatches, HitLine};
use super::plan::Plan;
use super::query::GrepMode;
use crate::HostError;

const MAX_STDOUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 64 * 1024;

pub(super) async fn search(
    plan: &Plan,
    rg: &Path,
    cancel: &CancellationToken,
) -> Result<Collected, HostError> {
    let mut child = tokio::process::Command::new(rg)
        .args(arguments(plan))
        .current_dir(&plan.root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| HostError::Process(format!("could not start ripgrep: {e}")))?;
    let stdout = child.stdout.take();
    let stderr = tokio::spawn(read_capped(child.stderr.take(), MAX_STDERR_BYTES, true));
    let (mut bytes, overflow) = tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(HostError::Cancelled),
        read = read_capped(stdout, MAX_STDOUT_BYTES, false) => read,
    };
    if overflow {
        if let Err(e) = child.start_kill() {
            tracing::debug!(error = %e, "ripgrep already exited");
        }
        let keep = bytes
            .iter()
            .rposition(|&b| b == b'\n' || b == 0)
            .map_or(0, |i| i + 1);
        bytes.truncate(keep);
    }
    let status = child
        .wait()
        .await
        .map_err(|e| HostError::Process(format!("waiting for ripgrep failed: {e}")))?;
    let (stderr, _) = stderr.await.unwrap_or_default();
    let stderr = String::from_utf8_lossy(&stderr);
    // 0: matches, 1: no matches, 2: errors (results may still be partial).
    if !matches!(status.code(), Some(0 | 1)) && !overflow && bytes.is_empty() {
        return Err(HostError::Process(format!(
            "ripgrep failed ({status}): {}",
            stderr.trim()
        )));
    }
    Ok(Collected {
        files: parse(&bytes, plan.query.mode),
        overflow,
    })
}

fn arguments(plan: &Plan) -> Vec<OsString> {
    let query = &plan.query;
    let mut args: Vec<OsString> = [
        "--no-config",
        "--null",
        "--no-heading",
        "--with-filename",
        "--line-number",
        "--color",
        "never",
        "--max-columns",
        "500",
        "--max-columns-preview",
        "--hidden",
        "--no-require-git",
        "--no-messages",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let mut push = |flag: &str, value: Option<String>| {
        args.push(flag.into());
        if let Some(value) = value {
            args.push(value.into());
        }
    };
    match query.mode {
        GrepMode::FilesWithMatches => push("--files-with-matches", None),
        GrepMode::Count => push("--count", None),
        GrepMode::Content => {
            if query.before > 0 {
                push("--before-context", Some(query.before.to_string()));
            }
            if query.after > 0 {
                push("--after-context", Some(query.after.to_string()));
            }
        }
    }
    push(
        if query.case_insensitive {
            "--ignore-case"
        } else {
            "--case-sensitive"
        },
        None,
    );
    if query.multiline {
        push("--multiline", None);
        push("--multiline-dotall", None);
    }
    if let Some(glob) = &query.glob {
        push("--glob", Some(glob.clone()));
    }
    // Last, so no user glob can re-include the repository directory.
    push("--glob", Some("!.git".to_string()));
    if let Some(file_type) = &query.file_type {
        push("--type", Some(file_type.clone()));
    }
    push("--regexp", Some(query.pattern.clone()));
    args.push("--".into());
    args.push(plan.rg_target().into_os_string());
    args
}

/// Reads up to `cap` bytes; with `drain`, keeps reading (and discarding)
/// so the child never blocks on a full pipe.
async fn read_capped<R>(pipe: Option<R>, cap: usize, drain: bool) -> (Vec<u8>, bool)
where
    R: AsyncRead + Unpin,
{
    let mut out = Vec::new();
    let Some(mut pipe) = pipe else {
        return (out, false);
    };
    let mut buf = vec![0u8; 64 * 1024];
    let mut overflow = false;
    loop {
        match pipe.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let room = cap.saturating_sub(out.len());
                out.extend_from_slice(&buf[..n.min(room)]);
                if n > room {
                    overflow = true;
                    if !drain {
                        break;
                    }
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "reading ripgrep output failed");
                break;
            }
        }
    }
    (out, overflow)
}

fn parse(bytes: &[u8], mode: GrepMode) -> Vec<FileMatches> {
    let text = String::from_utf8_lossy(bytes);
    match mode {
        GrepMode::FilesWithMatches => text
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(|path| FileMatches {
                path: display(path),
                lines: Vec::new(),
                count: 1,
            })
            .collect(),
        GrepMode::Count => text
            .lines()
            .filter_map(|line| {
                let (path, count) = line.split_once('\0')?;
                Some(FileMatches {
                    path: display(path),
                    lines: Vec::new(),
                    count: count.trim().parse().ok()?,
                })
            })
            .collect(),
        GrepMode::Content => parse_content(&text),
    }
}

/// `path\0N:text` for matches and `path\0N-text` for context; `--` group
/// separators and non-result lines are skipped (the renderer re-derives
/// separators).
fn parse_content(text: &str) -> Vec<FileMatches> {
    let mut files: Vec<FileMatches> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for line in text.split('\n') {
        let Some((path, rest)) = line.split_once('\0') else {
            continue;
        };
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let (number, tail) = rest.split_at(digits);
        let Ok(number) = number.parse::<u64>() else {
            continue;
        };
        let is_match = match tail.as_bytes().first() {
            Some(b':') => true,
            Some(b'-') => false,
            _ => continue,
        };
        let path = display(path);
        let slot = *index.entry(path.clone()).or_insert_with(|| {
            files.push(FileMatches {
                path,
                lines: Vec::new(),
                count: 0,
            });
            files.len() - 1
        });
        let file = &mut files[slot];
        file.count += usize::from(is_match);
        file.lines.push(HitLine {
            number,
            text: tail[1..].to_string(),
            is_match,
        });
    }
    files
}

/// ripgrep prefixes paths found under `.` with `./`.
fn display(path: &str) -> String {
    path.strip_prefix("./")
        .or_else(|| path.strip_prefix(".\\"))
        .unwrap_or(path)
        .to_string()
}
