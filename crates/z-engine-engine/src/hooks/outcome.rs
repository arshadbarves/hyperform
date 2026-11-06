//! How one hook run is read: exit 0 continues (JSON stdout may decide),
//! exit 2 blocks with stderr, anything else is a warning.

use serde::Deserialize;
use serde_json::Value;
use z_engine_host::RunOutput;

use super::event::HookEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermissionOverride {
    Allow,
    Ask,
    Deny,
}

/// The combined result of every hook that ran for one event.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct HookOutcome {
    /// A hook blocked the action, with its reason.
    pub blocked: Option<String>,
    /// `continue: false`: stop the agent, with the stop reason.
    pub stop: Option<String>,
    /// Extra context for the model (stdout or `additionalContext`).
    pub context: Vec<String>,
    /// `PreToolUse`: forced decision and its reason.
    pub permission: Option<(PermissionOverride, Option<String>)>,
    /// `PreToolUse`: replacement tool input.
    pub updated_input: Option<Value>,
}

/// What `HookRan` reports for one hook, plus a warning for a failed run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RunSummary {
    pub blocked: bool,
    pub message: Option<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct HookJson {
    decision: Option<String>,
    reason: Option<String>,
    #[serde(rename = "continue")]
    proceed: Option<bool>,
    stop_reason: Option<String>,
    hook_specific_output: Option<SpecificOutput>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SpecificOutput {
    permission_decision: Option<String>,
    permission_decision_reason: Option<String>,
    updated_input: Option<Value>,
    additional_context: Option<String>,
}

/// Folds one finished run into `outcome`.
pub(crate) fn interpret(
    event: HookEvent,
    run: &RunOutput,
    outcome: &mut HookOutcome,
) -> RunSummary {
    if run.timed_out {
        return warn("timed out".to_string());
    }
    match run.exit_code {
        Some(0) => success(event, run.stdout.trim(), outcome),
        Some(2) => {
            let reason = non_empty(run.stderr.trim()).unwrap_or("blocked by hook");
            outcome.blocked = Some(reason.to_string());
            RunSummary {
                blocked: true,
                message: Some(reason.to_string()),
                warning: None,
            }
        }
        Some(code) => warn(format!("exited with {code}: {}", run.stderr.trim())),
        None => warn("was stopped before it finished".to_string()),
    }
}

fn success(event: HookEvent, stdout: &str, outcome: &mut HookOutcome) -> RunSummary {
    let parsed = stdout
        .starts_with('{')
        .then(|| serde_json::from_str::<HookJson>(stdout).ok())
        .flatten();
    let Some(json) = parsed else {
        if event.stdout_is_context() && !stdout.is_empty() {
            outcome.context.push(stdout.to_string());
        }
        return RunSummary::default();
    };
    let mut summary = RunSummary::default();
    let specific = json.hook_specific_output.unwrap_or_default();
    if let Some(context) = specific.additional_context.as_deref().and_then(non_empty) {
        outcome.context.push(context.to_string());
    }
    if let Some(input) = specific.updated_input.filter(Value::is_object) {
        outcome.updated_input = Some(input);
    }
    let decision = specific
        .permission_decision
        .as_deref()
        .or(match json.decision.as_deref() {
            Some("approve") => Some("allow"),
            _ => None,
        });
    let forced = match decision {
        Some("allow") => Some(PermissionOverride::Allow),
        Some("ask") => Some(PermissionOverride::Ask),
        Some("deny") => Some(PermissionOverride::Deny),
        _ => None,
    };
    if let Some(forced) = forced {
        let reason = specific.permission_decision_reason.or(json.reason.clone());
        outcome.permission = Some(strictest(outcome.permission.take(), (forced, reason)));
    }
    if json.decision.as_deref() == Some("block") {
        let reason = json.reason.unwrap_or_else(|| "blocked by hook".to_string());
        summary.blocked = true;
        summary.message = Some(reason.clone());
        outcome.blocked = Some(reason);
    }
    if json.proceed == Some(false) {
        let reason = json
            .stop_reason
            .unwrap_or_else(|| "stopped by hook".to_string());
        summary.message = Some(reason.clone());
        outcome.stop = Some(reason);
    }
    summary
}

/// Deny beats ask beats allow.
fn strictest(
    current: Option<(PermissionOverride, Option<String>)>,
    next: (PermissionOverride, Option<String>),
) -> (PermissionOverride, Option<String>) {
    let rank = |forced: PermissionOverride| match forced {
        PermissionOverride::Allow => 0,
        PermissionOverride::Ask => 1,
        PermissionOverride::Deny => 2,
    };
    match current {
        Some(current) if rank(current.0) >= rank(next.0) => current,
        _ => next,
    }
}

fn warn(warning: String) -> RunSummary {
    RunSummary {
        blocked: false,
        message: Some(warning.clone()),
        warning: Some(warning),
    }
}

fn non_empty(text: &str) -> Option<&str> {
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(code: i32, stdout: &str, stderr: &str) -> RunOutput {
        RunOutput {
            exit_code: Some(code),
            stdout: stdout.into(),
            stderr: stderr.into(),
            ..RunOutput::default()
        }
    }

    #[test]
    fn exit_codes_continue_block_or_warn() {
        let mut outcome = HookOutcome::default();
        let summary = interpret(
            HookEvent::UserPromptSubmit,
            &run(0, "ctx", ""),
            &mut outcome,
        );
        assert_eq!(summary, RunSummary::default());
        assert_eq!(outcome.context, ["ctx"]);
        let summary = interpret(HookEvent::PreToolUse, &run(2, "", "no rm"), &mut outcome);
        assert!(summary.blocked);
        assert_eq!(outcome.blocked.as_deref(), Some("no rm"));
        let summary = interpret(HookEvent::Stop, &run(1, "", "boom"), &mut outcome);
        assert!(summary.warning.is_some_and(|w| w.contains("boom")));
    }

    #[test]
    fn json_output_decides() {
        let mut outcome = HookOutcome::default();
        let json = r#"{"hookSpecificOutput": {"permissionDecision": "deny",
            "permissionDecisionReason": "nope", "updatedInput": {"a": 1},
            "additionalContext": "note"}}"#;
        interpret(HookEvent::PreToolUse, &run(0, json, ""), &mut outcome);
        assert_eq!(
            outcome.permission,
            Some((PermissionOverride::Deny, Some("nope".into())))
        );
        assert_eq!(outcome.updated_input, Some(serde_json::json!({"a": 1})));
        assert_eq!(outcome.context, ["note"]);
        let allow = r#"{"decision": "approve"}"#;
        interpret(HookEvent::PreToolUse, &run(0, allow, ""), &mut outcome);
        assert_eq!(
            outcome.permission.as_ref().unwrap().0,
            PermissionOverride::Deny
        );
        let stop =
            r#"{"decision": "block", "reason": "tests", "continue": false, "stopReason": "done"}"#;
        let summary = interpret(HookEvent::Stop, &run(0, stop, ""), &mut outcome);
        assert!(summary.blocked);
        assert_eq!(outcome.blocked.as_deref(), Some("tests"));
        assert_eq!(outcome.stop.as_deref(), Some("done"));
    }
}
