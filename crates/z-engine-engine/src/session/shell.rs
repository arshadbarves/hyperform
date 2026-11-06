//! `!cmd`: a shell command the user runs directly, without the model. It
//! runs in the main agent's shell directory, reports its output as
//! `CommandOutput`, and counts as a change for the next turn's badge
//! unless it is recognisably read-only.

use std::sync::Arc;
use std::time::Duration;

use z_engine_host::{RunOutput, RunSpec, run};
use z_engine_policy::is_read_only;
use z_engine_protocol::Event;

use crate::session::SessionCore;
use crate::sync::lock;

const SHELL_TIMEOUT: Duration = Duration::from_secs(600);
const SHELL_OUTPUT_BYTES: usize = 1024 * 1024;
const SHOWN_OUTPUT_CHARS: usize = 20_000;

/// Runs in the background, owned by the session (cancelled on close).
pub(crate) fn spawn_shell(core: Arc<SessionCore>, command: String) {
    let command = command.trim().to_string();
    if command.is_empty() {
        return;
    }
    tokio::spawn(async move {
        let settings = core.settings();
        let spec = RunSpec {
            command: command.clone(),
            cwd: core.main.cwd(),
            timeout: SHELL_TIMEOUT,
            shell: settings.shell.clone(),
            env: settings.env.clone(),
            track_cwd: true,
            max_output_bytes: SHELL_OUTPUT_BYTES,
            stdin: None,
        };
        let result = run(spec, core.cancel.child_token(), None).await;
        if !is_read_only(&command) {
            core.with_state(|state| state.external_mutation = true);
        }
        let markdown = match result {
            Ok(output) => {
                if let Some(dir) = &output.final_cwd {
                    *lock(&core.main.cwd) = dir.clone();
                }
                render(&command, &output)
            }
            Err(error) => {
                format!("```console\n$ {command}\n```\nCould not run the command: {error}")
            }
        };
        core.events.emit(Event::CommandOutput {
            name: "shell".to_string(),
            markdown,
        });
    });
}

fn render(command: &str, output: &RunOutput) -> String {
    let combined = output.combined.trim_end();
    let total = combined.chars().count();
    let shown: String = if total > SHOWN_OUTPUT_CHARS {
        let skip = total - SHOWN_OUTPUT_CHARS;
        format!(
            "… {skip} characters omitted …\n{}",
            combined.chars().skip(skip).collect::<String>()
        )
    } else {
        combined.to_string()
    };
    let status = if output.timed_out {
        "timed out".to_string()
    } else if output.cancelled {
        "cancelled".to_string()
    } else {
        match output.exit_code {
            Some(code) => format!("exit code {code}"),
            None => "killed".to_string(),
        }
    };
    let fence = if shown.contains("```") { "````" } else { "```" };
    format!("{fence}console\n$ {command}\n{shown}\n{fence}\n{status}")
}
