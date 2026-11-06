//! Rule precedence and matching for non-shell actions.

mod support;

use support::Outcome::{Allow, Ask, Deny};
use support::*;
use z_engine_policy::{Action, Policy, PolicyError};

const DEFAULT: PermissionMode = PermissionMode::Default;

#[test]
fn deny_beats_allow_and_session_rules() {
    let mut policy = policy(
        &["Edit", "Read"],
        &[],
        &["Edit(secrets/**)", "Read(//etc/**)"],
    );
    policy.add_session_rule("Edit(secrets/**)").unwrap();
    let edit = policy.decide("Edit", &write(&["/work/proj/secrets/key"]), false, DEFAULT);
    assert_eq!(outcome(&edit), Deny);
    assert_eq!(edit.reason(), "denied by rule Edit(secrets/**)");
    assert_eq!(
        outcome(&policy.decide("Read", &read(&["/etc/passwd"]), false, DEFAULT)),
        Deny
    );
    assert_eq!(
        outcome(&policy.decide("Edit", &write(&["/work/proj/src/a.rs"]), false, DEFAULT)),
        Allow
    );
}

#[test]
fn ask_beats_allow() {
    let policy = policy(&["Edit"], &["Edit(docs/**)", "Read(//var/log/**)"], &[]);
    let docs = policy.decide(
        "Write",
        &write(&["/work/proj/docs/guide.md"]),
        false,
        DEFAULT,
    );
    assert_eq!(outcome(&docs), Ask);
    assert_eq!(docs.reason(), "rule Edit(docs/**) requires approval");
    assert!(persistable(&docs));
    assert_eq!(suggested(&docs), None);
    let logs = policy.decide("Read", &read(&["/var/log/system.log"]), false, DEFAULT);
    assert_eq!(outcome(&logs), Ask);
    assert!(!persistable(&logs));
    assert_eq!(
        outcome(&policy.decide("Edit", &write(&["/work/proj/src/a.rs"]), false, DEFAULT)),
        Allow
    );
}

#[test]
fn edit_and_read_rules_cover_their_tool_groups() {
    let policy = policy(&["Edit(src/**)"], &[], &["Read(~/.ssh/**)"]);
    for tool in [
        "Write",
        "Edit",
        "MultiEdit",
        "NotebookEdit",
        "ApplyAgentChanges",
    ] {
        let decision = policy.decide(tool, &write(&["/work/proj/src/lib.rs"]), false, DEFAULT);
        assert_eq!(outcome(&decision), Allow, "{tool}");
    }
    for tool in ["Read", "Glob", "Grep", "LSP"] {
        let decision = policy.decide(
            tool,
            &read(&["/home/me/.ssh/id_rsa"]),
            false,
            PermissionMode::Bypass,
        );
        assert_eq!(outcome(&decision), Deny, "{tool}");
    }
    let listing = policy.decide("Glob", &read(&["/home/me/.ssh"]), false, DEFAULT);
    assert_eq!(outcome(&listing), Deny);
    assert_eq!(
        outcome(&policy.decide("Read", &read(&["/work/proj/src/lib.rs"]), false, DEFAULT)),
        Allow
    );
}

#[test]
fn multi_path_actions_need_every_path_covered() {
    let policy = policy(&["Edit(src/**)"], &[], &["Edit(**/*.lock)"]);
    let both = write(&["/work/proj/src/a.rs", "/work/proj/docs/b.md"]);
    let decision = policy.decide("ApplyAgentChanges", &both, false, DEFAULT);
    assert_eq!(outcome(&decision), Ask);
    assert_eq!(decision.reason(), "edits src/a.rs and 1 more");
    let locked = write(&["/work/proj/src/a.rs", "/work/proj/Cargo.lock"]);
    assert_eq!(
        outcome(&policy.decide("ApplyAgentChanges", &locked, false, DEFAULT)),
        Deny
    );
}

#[test]
fn bare_group_rules_stay_inside_allowed_directories() {
    let policy = policy(&["Edit", "Read"], &[], &[]);
    let outside = policy.decide("Write", &write(&["/tmp/scratch.txt"]), false, DEFAULT);
    assert_eq!(outcome(&outside), Ask);
    assert!(!persistable(&outside));
    assert_eq!(
        outcome(&policy.decide("Read", &read(&["/etc/hosts"]), false, DEFAULT)),
        Ask
    );
    let traversal = policy.decide(
        "Write",
        &write(&["/work/proj/../elsewhere/x"]),
        false,
        DEFAULT,
    );
    assert_eq!(outcome(&traversal), Ask);
    let protected = policy.decide("Edit", &write(&["/work/proj/.git/config"]), false, DEFAULT);
    assert_eq!(outcome(&protected), Ask);
    let explicit = policy.decide(
        "Edit",
        &write(&["/work/proj/.claude/settings.json"]),
        false,
        DEFAULT,
    );
    assert_eq!(outcome(&explicit), Ask);
}

#[test]
fn explicit_patterns_reach_outside_the_project() {
    let policy = policy(
        &["Edit(//tmp/**)", "Read(~/.cargo/**)", "Grep(/opt/src/**)"],
        &[],
        &[],
    );
    assert_eq!(
        outcome(&policy.decide("Write", &write(&["/tmp/a/b.txt"]), false, DEFAULT)),
        Allow
    );
    let registry = read(&["/home/me/.cargo/registry/src/serde/lib.rs"]);
    assert_eq!(
        outcome(&policy.decide("Read", &registry, false, DEFAULT)),
        Allow
    );
    assert_eq!(
        outcome(&policy.decide("Grep", &read(&["/opt/src/x.c"]), false, DEFAULT)),
        Allow
    );
    assert_eq!(
        outcome(&policy.decide("Read", &read(&["/opt/src/x.c"]), false, DEFAULT)),
        Ask
    );
}

#[test]
fn single_slash_patterns_also_read_as_project_relative() {
    let policy = policy(&[], &[], &["Read(/secrets/**)"]);
    let project = policy.decide(
        "Read",
        &read(&["/work/proj/secrets/token"]),
        false,
        PermissionMode::Bypass,
    );
    assert_eq!(outcome(&project), Deny);
    let absolute = policy.decide(
        "Read",
        &read(&["/secrets/token"]),
        false,
        PermissionMode::Bypass,
    );
    assert_eq!(outcome(&absolute), Deny);
}

#[test]
fn web_fetch_domains_include_subdomains() {
    let policy = policy(
        &["WebFetch(domain:docs.rs)", "WebFetch(domain:*.github.com)"],
        &[],
        &["WebFetch(domain:evil.io)"],
    );
    let decide = |url: &str| outcome(&policy.decide("WebFetch", &fetch(url), false, DEFAULT));
    assert_eq!(decide("https://docs.rs/serde"), Allow);
    assert_eq!(decide("https://api.docs.rs/x"), Allow);
    assert_eq!(decide("https://github.com/x"), Allow);
    assert_eq!(decide("https://raw.github.com/x"), Allow);
    assert_eq!(decide("https://notdocs.rs/"), Ask);
    assert_eq!(decide("https://docs.rs.evil.io/"), Deny);
    assert_eq!(decide("https://docs.rs@evil.io/"), Deny);
    assert_eq!(decide("https://%65vil.io/"), Deny);
    assert_eq!(decide("file:///etc/passwd"), Ask);
}

#[test]
fn mcp_rules_and_wildcards() {
    let policy = policy(
        &["mcp__github"],
        &["mcp__github__merge_pr"],
        &["mcp__github__delete_repo", "mcp__prod__*"],
    );
    let call = |tool: &str| {
        let action = mcp("github", tool, false);
        outcome(&policy.decide(&format!("mcp__github__{tool}"), &action, false, DEFAULT))
    };
    assert_eq!(call("create_issue"), Allow);
    assert_eq!(call("merge_pr"), Ask);
    assert_eq!(call("delete_repo"), Deny);
    let prod = policy.decide(
        "mcp__prod__query",
        &mcp("prod", "query", true),
        false,
        PermissionMode::Bypass,
    );
    assert_eq!(outcome(&prod), Deny);
    let other = policy.decide(
        "mcp__linear__list",
        &mcp("linear", "list", true),
        false,
        DEFAULT,
    );
    assert_eq!(outcome(&other), Ask);
}

#[test]
fn agent_skill_and_bare_tool_rules() {
    let policy = policy(
        &["JobKill"],
        &[],
        &["Task(general-purpose)", "Skill(deploy)", "TodoWrite"],
    );
    let agent = |kind: &str| Action::Agent {
        agent_type: kind.into(),
    };
    assert_eq!(
        outcome(&policy.decide("Agent", &agent("general-purpose"), false, DEFAULT)),
        Deny
    );
    assert_eq!(
        outcome(&policy.decide("Agent", &agent("explore"), false, DEFAULT)),
        Allow
    );
    let skill = |name: &str| Action::Skill { name: name.into() };
    assert_eq!(
        outcome(&policy.decide("Skill", &skill("deploy"), false, DEFAULT)),
        Deny
    );
    assert_eq!(
        outcome(&policy.decide("Skill", &skill("pdf"), false, DEFAULT)),
        Allow
    );
    let kill = Action::Other { read_only: false };
    assert_eq!(
        outcome(&policy.decide("JobKill", &kill, false, DEFAULT)),
        Allow
    );
    let todo = Action::Other { read_only: true };
    assert_eq!(
        outcome(&policy.decide("TodoWrite", &todo, false, DEFAULT)),
        Deny
    );
}

#[test]
fn session_rules_grant_for_the_rest_of_the_session() {
    let mut policy = policy(&[], &[], &[]);
    let edit = write(&["/work/proj/src/lib.rs"]);
    assert_eq!(outcome(&policy.decide("Edit", &edit, false, DEFAULT)), Ask);
    policy.add_session_rule("Edit").unwrap();
    assert_eq!(
        outcome(&policy.decide("Edit", &edit, false, DEFAULT)),
        Allow
    );
    assert_eq!(
        outcome(&policy.decide("Edit", &write(&["/tmp/x"]), false, DEFAULT)),
        Ask
    );
    policy
        .add_session_rule("WebFetch(domain:example.com)")
        .unwrap();
    assert_eq!(
        outcome(&policy.decide("WebFetch", &fetch("https://example.com/a"), false, DEFAULT)),
        Allow
    );
    assert_eq!(
        policy.session_rules(),
        ["Edit", "WebFetch(domain:example.com)"]
    );
}

#[test]
fn invalid_rules_are_reported_while_valid_ones_apply() {
    let settings = config(
        &["Bash(npm test:*)", "Read("],
        &[],
        &["Edit(src/[)", "Bash(rm:*)"],
    );
    let (policy, errors) = Policy::new(&settings, ctx());
    assert_eq!(errors.len(), 2);
    assert!(matches!(errors[0], PolicyError::InvalidRule { .. }));
    assert!(matches!(errors[1], PolicyError::InvalidPattern { .. }));
    assert_eq!(outcome(&bash(&policy, "npm test", DEFAULT)), Allow);
    assert_eq!(
        outcome(&bash(&policy, "rm x", PermissionMode::Bypass)),
        Deny
    );
}
