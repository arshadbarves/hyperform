//! Sandbox auto-allow precedence: deny rules, plan mode, and ask rules
//! still win over `sandboxed`; allow rules and read-only keep their own
//! reasons.

mod support;

use hyperform_policy::{Decision, Policy, PolicyConfig};
use support::{LEVELS, Outcome, PermissionMode, bash, config, ctx, outcome, plan_bash};

fn sandboxed(allow: &[&str], ask: &[&str], deny: &[&str]) -> Policy {
    let config = PolicyConfig {
        sandbox_auto_allow: true,
        ..config(allow, ask, deny)
    };
    let (policy, errors) = Policy::new(&config, ctx());
    assert!(errors.is_empty(), "{errors:?}");
    policy
}

fn reason(decision: &Decision) -> &str {
    decision.reason()
}

#[test]
fn deny_rules_still_deny() {
    let policy = sandboxed(&[], &[], &["Bash(git push:*)", "Edit(./secrets/**)"]);
    let push = bash(
        &policy,
        "cargo build && git push origin main",
        PermissionMode::Default,
    );
    assert_eq!(outcome(&push), Outcome::Deny);
    let write = bash(&policy, "echo x > secrets/key", PermissionMode::Default);
    assert_eq!(outcome(&write), Outcome::Deny);
}

#[test]
fn plan_mode_still_denies_mutations() {
    let policy = sandboxed(&[], &[], &[]);
    for level in LEVELS {
        let build = plan_bash(&policy, "cargo build", level);
        assert_eq!(outcome(&build), Outcome::Deny, "{level:?}");
    }
    let status = plan_bash(&policy, "git status", PermissionMode::Default);
    assert_eq!(outcome(&status), Outcome::Allow);
    assert_eq!(reason(&status), "read-only command");
}

#[test]
fn ask_rules_still_ask() {
    let policy = sandboxed(&[], &["Bash(npm publish:*)"], &[]);
    let publish = bash(&policy, "npm publish", PermissionMode::Default);
    assert_eq!(outcome(&publish), Outcome::Ask);
    let install = bash(&policy, "npm install", PermissionMode::Default);
    assert_eq!(reason(&install), "sandboxed");
}

#[test]
fn allow_rules_and_read_only_keep_their_reasons() {
    let policy = sandboxed(&["Bash(cargo test:*)"], &[], &[]);
    let test = bash(&policy, "cargo test", PermissionMode::Default);
    assert_eq!(reason(&test), "allowed by rule Bash(cargo test:*)");
    let ls = bash(&policy, "ls src", PermissionMode::Default);
    assert_eq!(reason(&ls), "read-only command");
    let make = bash(&policy, "make all", PermissionMode::Default);
    assert_eq!(reason(&make), "sandboxed");
}

#[test]
fn additional_dirs_count_as_inside_and_other_writes_ask() {
    let policy = sandboxed(&[], &[], &[]);
    let shared = bash(
        &policy,
        "touch /data/shared/out.csv",
        PermissionMode::Default,
    );
    assert_eq!(reason(&shared), "sandboxed");
    let home = bash(&policy, "touch ~/notes.txt", PermissionMode::Default);
    assert_eq!(outcome(&home), Outcome::Ask);
    let dynamic = bash(&policy, "rm -rf $(cat list)", PermissionMode::Default);
    assert_eq!(outcome(&dynamic), Outcome::Ask);
}
