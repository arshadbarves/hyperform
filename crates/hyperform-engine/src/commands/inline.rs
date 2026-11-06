//! `` !`cmd` `` in command bodies: run in the project root before the
//! prompt is sent, only when the command's `allowed-tools` or the session
//! policy already allow it; otherwise a note says it needs approval.

use std::time::Duration;

use hyperform_host::{RunOutput, RunSpec, run};
use hyperform_policy::{Action, Decision, is_read_only};
use tokio_util::sync::CancellationToken;

use super::mentions::inline_commands;
use crate::session::SessionCore;
use crate::sync::lock;

const INLINE_TIMEOUT: Duration = Duration::from_secs(120);
const INLINE_OUTPUT_BYTES: usize = 64 * 1024;

/// `body` with every inline command replaced by its output or a note.
pub(crate) async fn run_inline_commands(
    core: &SessionCore,
    body: &str,
    allowed_tools: &[String],
    cancel: &CancellationToken,
) -> String {
    let mut out = body.to_string();
    for (span, command) in inline_commands(body) {
        let replacement = if permitted(core, allowed_tools, &command) {
            execute(core, &command, cancel).await
        } else {
            format!(
                "[`{command}` was not run: it needs approval. Allow it in the command's \
                 allowed-tools or in the permission settings.]"
            )
        };
        out = out.replacen(&span, &replacement, 1);
    }
    out
}

/// The session policy plus the command's grants decides, as for `Bash`.
pub(crate) fn permitted(core: &SessionCore, allowed_tools: &[String], command: &str) -> bool {
    let mut policy = lock(&core.policy).clone();
    for rule in allowed_tools {
        if let Err(error) = policy.add_session_rule(rule) {
            tracing::debug!(%rule, %error, "invalid allowed-tools rule ignored");
        }
    }
    let action = Action::Execute {
        command: command.to_string(),
    };
    matches!(
        policy.decide("Bash", &action, core.planning(), core.mode()),
        Decision::Allow { .. }
    )
}

async fn execute(core: &SessionCore, command: &str, cancel: &CancellationToken) -> String {
    let settings = core.settings();
    let mut spec = RunSpec::new(command, core.root.clone());
    spec.timeout = INLINE_TIMEOUT;
    spec.shell = settings.shell.clone();
    spec.env = settings.env.clone();
    spec.track_cwd = false;
    spec.max_output_bytes = INLINE_OUTPUT_BYTES;
    let result = run(spec, cancel.child_token(), None).await;
    if !is_read_only(command) {
        core.with_state(|state| state.external_mutation = true);
    }
    match result {
        Ok(output) => render(command, &output),
        Err(error) => format!("[`{command}` could not run: {error}]"),
    }
}

fn render(command: &str, output: &RunOutput) -> String {
    let text = output.combined.trim_end();
    let fence = if text.contains("```") { "````" } else { "```" };
    let status = match (output.timed_out, output.exit_code) {
        (true, _) => "\n(timed out)".to_string(),
        (false, Some(0)) => String::new(),
        (false, Some(code)) => format!("\n(exit code {code})"),
        (false, None) => "\n(killed)".to_string(),
    };
    format!("{fence}console\n$ {command}\n{text}\n{fence}{status}")
}
