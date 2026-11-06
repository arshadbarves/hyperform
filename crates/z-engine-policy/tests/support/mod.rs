//! Shared fixtures for the policy integration tests. Each test crate uses a
//! different subset, hence the `dead_code` allowance.
#![allow(dead_code)]

use std::path::PathBuf;

use z_engine_policy::{Action, Decision, Policy, PolicyConfig, PolicyContext};
pub use z_engine_protocol::PermissionMode;

pub const LEVELS: [PermissionMode; 3] = [
    PermissionMode::Default,
    PermissionMode::AcceptEdits,
    PermissionMode::Bypass,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Allow,
    Ask,
    Deny,
}

pub fn outcome(decision: &Decision) -> Outcome {
    match decision {
        Decision::Allow { .. } => Outcome::Allow,
        Decision::Ask { .. } => Outcome::Ask,
        Decision::Deny { .. } => Outcome::Deny,
    }
}

pub fn ctx() -> PolicyContext {
    PolicyContext {
        project_root: PathBuf::from("/work/proj"),
        additional_dirs: vec![PathBuf::from("/data/shared")],
        home: Some(PathBuf::from("/home/me")),
    }
}

pub fn config(allow: &[&str], ask: &[&str], deny: &[&str]) -> PolicyConfig {
    let strings = |rules: &[&str]| rules.iter().map(|rule| rule.to_string()).collect();
    PolicyConfig {
        allow: strings(allow),
        ask: strings(ask),
        deny: strings(deny),
        auto_allow_read_only_bash: true,
        sandbox_auto_allow: false,
    }
}

pub fn policy(allow: &[&str], ask: &[&str], deny: &[&str]) -> Policy {
    let (policy, errors) = Policy::new(&config(allow, ask, deny), ctx());
    assert!(errors.is_empty(), "{errors:?}");
    policy
}

pub fn read(paths: &[&str]) -> Action {
    Action::Read {
        paths: paths.iter().map(PathBuf::from).collect(),
    }
}

pub fn write(paths: &[&str]) -> Action {
    Action::Write {
        paths: paths.iter().map(PathBuf::from).collect(),
    }
}

pub fn exec(command: &str) -> Action {
    Action::Execute {
        command: command.to_string(),
    }
}

pub fn fetch(url: &str) -> Action {
    Action::Fetch {
        url: url.to_string(),
    }
}

pub fn mcp(server: &str, tool: &str, read_only: bool) -> Action {
    Action::Mcp {
        server: server.to_string(),
        tool: tool.to_string(),
        read_only,
    }
}

/// Decides a `Bash` call outside plan mode.
pub fn bash(policy: &Policy, command: &str, mode: PermissionMode) -> Decision {
    policy.decide("Bash", &exec(command), false, mode)
}

/// Decides a `Bash` call in plan mode.
pub fn plan_bash(policy: &Policy, command: &str, mode: PermissionMode) -> Decision {
    policy.decide("Bash", &exec(command), true, mode)
}

pub fn suggested(decision: &Decision) -> Option<&str> {
    match decision {
        Decision::Ask { suggested_rule, .. } => suggested_rule.as_deref(),
        _ => None,
    }
}

pub fn persistable(decision: &Decision) -> bool {
    matches!(
        decision,
        Decision::Ask {
            can_persist: true,
            ..
        }
    )
}
