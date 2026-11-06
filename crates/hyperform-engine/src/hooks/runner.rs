//! Runs the configured hooks of one event in order. Every run emits
//! `HookRan`; failed runs also warn. The first block or stop ends the
//! chain, and a rewritten tool input is what later hooks see.

use std::time::Duration;

use hyperform_config::HookConfig;
use hyperform_host::{HostError, RunOutput, RunSpec, run};
use hyperform_protocol::{Event, NoticeLevel};
use regex::Regex;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::event::{HookEnv, HookEvent, HookInput};
use super::outcome::{HookOutcome, RunSummary, interpret};
use crate::session::Emitter;

const HOOK_OUTPUT_LIMIT: usize = 256 * 1024;

pub(crate) async fn run_hooks(
    env: &HookEnv,
    events: &Emitter,
    event: HookEvent,
    mut input: HookInput,
    cancel: &CancellationToken,
) -> HookOutcome {
    let mut outcome = HookOutcome::default();
    let Some(hooks) = env.settings.settings.hooks.get(event.name()) else {
        return outcome;
    };
    for hook in hooks.iter().filter(|hook| !hook.command.trim().is_empty()) {
        match matches(hook.matcher.as_deref(), input.target.as_deref()) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => {
                let matcher = hook.matcher.as_deref().unwrap_or_default();
                events.notice(
                    NoticeLevel::Warn,
                    format!(
                        "{} hook matcher `{matcher}` is invalid: {error}",
                        event.name()
                    ),
                );
                continue;
            }
        }
        let payload = env.payload(event, &input);
        let summary = match run_one(env, hook, &payload, cancel).await {
            Ok(output) => interpret(event, &output, &mut outcome),
            Err(error) => failed(&error),
        };
        if let Some(warning) = &summary.warning {
            events.notice(
                NoticeLevel::Warn,
                format!("{} hook `{}` {warning}", event.name(), hook.command),
            );
        }
        events.emit(Event::HookRan {
            hook_event: event.name().to_string(),
            command: hook.command.clone(),
            blocked: summary.blocked,
            message: summary.message,
        });
        if let Some(updated) = &outcome.updated_input {
            input.fields.insert("tool_input".into(), updated.clone());
        }
        if outcome.blocked.is_some() || outcome.stop.is_some() {
            break;
        }
    }
    outcome
}

/// `None`, empty and `*` match everything, as does any matcher for events
/// without a target; otherwise the regex must match the whole target.
fn matches(matcher: Option<&str>, target: Option<&str>) -> Result<bool, regex::Error> {
    let pattern = matcher.map(str::trim).unwrap_or_default();
    if pattern.is_empty() || pattern == "*" {
        return Ok(true);
    }
    let Some(target) = target else {
        return Ok(true);
    };
    Ok(Regex::new(&format!("^(?:{pattern})$"))?.is_match(target))
}

async fn run_one(
    env: &HookEnv,
    hook: &HookConfig,
    payload: &Value,
    cancel: &CancellationToken,
) -> Result<RunOutput, HostError> {
    let stdin = serde_json::to_vec(payload).map_err(|e| HostError::Invalid(e.to_string()))?;
    let mut policy = env.settings.env.clone();
    let project_dir = env.cwd.to_string_lossy().into_owned();
    policy
        .extra
        .insert("HYPERFORM_PROJECT_DIR".into(), project_dir.clone());
    policy
        .extra
        .insert("CLAUDE_PROJECT_DIR".into(), project_dir);
    let spec = RunSpec {
        command: hook.command.clone(),
        cwd: env.cwd.clone(),
        timeout: Duration::from_secs(hook.timeout_secs.max(1)),
        shell: env.settings.shell.clone(),
        env: policy,
        track_cwd: false,
        max_output_bytes: HOOK_OUTPUT_LIMIT,
        stdin: Some(stdin),
    };
    run(spec, cancel.clone(), None).await
}

fn failed(error: &HostError) -> RunSummary {
    let warning = format!("could not run: {error}");
    RunSummary {
        blocked: false,
        message: Some(warning.clone()),
        warning: Some(warning),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matchers_are_anchored_regexes() {
        assert!(matches(None, Some("Bash")).unwrap());
        assert!(matches(Some("*"), Some("Bash")).unwrap());
        assert!(matches(Some("Edit|Write"), Some("Write")).unwrap());
        assert!(!matches(Some("Edit"), Some("NotebookEdit")).unwrap());
        assert!(matches(Some("Notebook.*"), Some("NotebookEdit")).unwrap());
        assert!(matches(Some("Bash"), None).unwrap());
        assert!(matches(Some("("), Some("Bash")).is_err());
    }
}
