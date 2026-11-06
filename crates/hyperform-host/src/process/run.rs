//! One-shot shell commands: live line streaming, a deadline, cancellation,
//! whole-tree cleanup, bounded capture, and a persistent working directory.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWriteExt};
use tokio::process::ChildStdin;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::capture::{Capture, Stream};
use super::env::EnvPolicy;
use super::lines::pump;
use super::script::CwdProbe;
use super::shell::{ShellSpec, resolve_shell};
use super::spawn::{check_command, drain, shell_command, spawn_error, terminate};
use super::tree::TreeGuard;
use crate::HostError;

/// Receives stdout and stderr lines (with their terminator) as they arrive.
pub type OutputSink = Arc<dyn Fn(&str) + Send + Sync>;

pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
pub const DEFAULT_RUN_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct RunSpec {
    pub command: String,
    pub cwd: PathBuf,
    pub timeout: Duration,
    pub shell: ShellSpec,
    pub env: EnvPolicy,
    /// Wrap the command so `final_cwd` reports where the shell ended up.
    pub track_cwd: bool,
    /// Per-stream capture budget; beyond it the middle is dropped.
    pub max_output_bytes: usize,
    /// Written in full to the child's stdin, which is then closed (e.g. a
    /// hook's JSON payload). `None` leaves stdin closed.
    pub stdin: Option<Vec<u8>>,
}

impl RunSpec {
    /// Detected shell, default environment policy, 120 s timeout, cwd
    /// tracking on, 4 MiB capture budget.
    pub fn new(command: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            command: command.into(),
            cwd: cwd.into(),
            timeout: DEFAULT_RUN_TIMEOUT,
            shell: resolve_shell(None),
            env: EnvPolicy::default(),
            track_cwd: true,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            stdin: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunOutput {
    /// `None` when the shell was killed by a signal (timeout, cancel).
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// stdout and stderr interleaved in arrival order.
    pub combined: String,
    pub timed_out: bool,
    pub cancelled: bool,
    /// Some output exceeded `max_output_bytes` and was dropped.
    pub truncated: bool,
    pub duration_ms: u64,
    /// The shell's final directory (only when tracked and it exited itself).
    pub final_cwd: Option<PathBuf>,
}

enum Ending {
    Exited(std::process::ExitStatus),
    TimedOut,
    Cancelled,
}

/// Runs `spec.command` to completion, timeout, or cancellation. Returns
/// `HostError::Cancelled` only when cancelled before anything started;
/// a later cancellation or timeout is reported in the output with whatever
/// was captured. The whole process tree is killed on timeout and cancel.
pub async fn run(
    spec: RunSpec,
    cancel: CancellationToken,
    on_output: Option<OutputSink>,
) -> Result<RunOutput, HostError> {
    check_command(&spec.command, &spec.cwd)?;
    if cancel.is_cancelled() {
        return Err(HostError::Cancelled);
    }
    let probe = spec.track_cwd.then(CwdProbe::new);
    let script = match &probe {
        Some(probe) => probe.wrap(spec.shell.kind, &spec.command),
        None => spec.command.clone(),
    };
    let started = Instant::now();
    let mut command = shell_command(&spec.shell, &script, &spec.cwd, &spec.env);
    if spec.stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().map_err(|e| spawn_error(&spec.shell, e))?;
    let pid = child.id();
    let tree = TreeGuard::new(pid);
    let writer = match (spec.stdin, child.stdin.take()) {
        (Some(input), Some(pipe)) => Some(tokio::spawn(feed_stdin(pipe, input))),
        _ => None,
    };
    let capture = Arc::new(Mutex::new(Capture::new(spec.max_output_bytes)));
    let mut readers = vec![
        reader(child.stdout.take(), Stream::Stdout, &capture, &on_output),
        reader(child.stderr.take(), Stream::Stderr, &capture, &on_output),
    ];

    let ending = tokio::select! {
        status = child.wait() => Ending::Exited(
            status.map_err(|e| HostError::Process(format!("waiting for the shell failed: {e}")))?,
        ),
        () = cancel.cancelled() => Ending::Cancelled,
        () = tokio::time::sleep(spec.timeout) => Ending::TimedOut,
    };
    let status = match ending {
        Ending::Exited(status) => {
            drain(&mut readers, pid, Some(&cancel)).await;
            Some(status)
        }
        Ending::TimedOut | Ending::Cancelled => {
            terminate(&mut child, pid).await;
            drain(&mut readers, pid, None).await;
            None
        }
    };
    tree.disarm();
    // The tree is gone, so the pipe is closed; a writer still pending was
    // blocked on a reader that no longer exists.
    if let Some(writer) = writer {
        writer.abort();
    }

    // An aborted reader may still hold its clone, so take the contents.
    let captured = std::mem::replace(
        &mut *capture.lock().unwrap_or_else(PoisonError::into_inner),
        Capture::new(0),
    )
    .finish();
    let final_cwd = status.and_then(|_| probe.as_ref()?.read());
    Ok(RunOutput {
        exit_code: status.and_then(|s| s.code()),
        stdout: captured.stdout,
        stderr: captured.stderr,
        combined: captured.combined,
        timed_out: matches!(ending, Ending::TimedOut),
        cancelled: matches!(ending, Ending::Cancelled),
        truncated: captured.truncated,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        final_cwd,
    })
}

/// Writes `input` and closes stdin. A child that exits without reading
/// closes the pipe, which is not an error.
async fn feed_stdin(mut pipe: ChildStdin, input: Vec<u8>) {
    let written = async {
        pipe.write_all(&input).await?;
        pipe.shutdown().await
    };
    match written.await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(e) => tracing::debug!(error = %e, "writing stdin failed"),
    }
}

fn reader<R>(
    pipe: Option<R>,
    stream: Stream,
    capture: &Arc<Mutex<Capture>>,
    sink: &Option<OutputSink>,
) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let capture = Arc::clone(capture);
    let sink = sink.clone();
    tokio::spawn(async move {
        let Some(pipe) = pipe else { return };
        pump(pipe, move |line| {
            capture
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(stream, line);
            if let Some(sink) = &sink {
                sink(line);
            }
        })
        .await;
    })
}
