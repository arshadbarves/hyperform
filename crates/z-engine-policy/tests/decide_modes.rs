//! Defaults for every permission level, with and without plan mode, and
//! every action, without any rules.

mod support;

use support::Outcome::{Allow, Ask, Deny};
use support::*;
use z_engine_policy::{Action, Decision, Policy, PolicyConfig};

/// `(planning, level)` in the column order of the expectations below.
const COLUMNS: [(bool, PermissionMode); 6] = [
    (false, PermissionMode::Default),
    (false, PermissionMode::AcceptEdits),
    (false, PermissionMode::Bypass),
    (true, PermissionMode::Default),
    (true, PermissionMode::AcceptEdits),
    (true, PermissionMode::Bypass),
];

const PLAN_DENIAL: &str = "plan mode is read-only; use ExitPlanMode to propose the plan";

#[test]
fn every_level_with_and_without_planning_and_every_action() {
    let policy = policy(&[], &[], &[]);
    // Columns: default, acceptEdits, bypass, then the same levels planning.
    let cases: Vec<(&str, Action, [Outcome; 6])> = vec![
        ("Read", read(&["/work/proj/src/lib.rs"]), [Allow; 6]),
        ("Glob", read(&[]), [Allow; 6]),
        ("Grep", read(&["/data/shared/table.csv"]), [Allow; 6]),
        (
            "Read",
            read(&["/etc/hosts"]),
            [Ask, Ask, Allow, Ask, Ask, Allow],
        ),
        (
            "LSP",
            read(&["/work/proj/src", "/etc"]),
            [Ask, Ask, Allow, Ask, Ask, Allow],
        ),
        (
            "Edit",
            write(&["/work/proj/src/lib.rs"]),
            [Ask, Allow, Allow, Deny, Deny, Deny],
        ),
        (
            "MultiEdit",
            write(&["/data/shared/a", "/work/proj/b"]),
            [Ask, Allow, Allow, Deny, Deny, Deny],
        ),
        (
            "Write",
            write(&["/tmp/out.txt"]),
            [Ask, Ask, Allow, Deny, Deny, Deny],
        ),
        (
            "Edit",
            write(&["/work/proj/.git/config"]),
            [Ask, Ask, Allow, Deny, Deny, Deny],
        ),
        (
            "Write",
            write(&["/work/proj/.hyperform/settings.json"]),
            [Ask, Ask, Allow, Deny, Deny, Deny],
        ),
        (
            "ApplyAgentChanges",
            write(&[]),
            [Ask, Ask, Allow, Deny, Deny, Deny],
        ),
        ("Bash", exec("git status && ls -la"), [Allow; 6]),
        (
            "Bash",
            exec("cat /etc/hosts"),
            [Ask, Ask, Allow, Ask, Ask, Allow],
        ),
        (
            "Bash",
            exec("cargo test"),
            [Ask, Ask, Allow, Deny, Deny, Deny],
        ),
        (
            "Bash",
            exec("mkdir -p src/new && touch src/new/mod.rs"),
            [Ask, Allow, Allow, Deny, Deny, Deny],
        ),
        (
            "Bash",
            exec("rm -rf /"),
            [Ask, Ask, Allow, Deny, Deny, Deny],
        ),
        (
            "Bash",
            exec("echo $(date)"),
            [Ask, Ask, Allow, Deny, Deny, Deny],
        ),
        (
            "WebFetch",
            fetch("https://example.com/docs"),
            [Ask, Ask, Allow, Ask, Ask, Allow],
        ),
        (
            "WebSearch",
            Action::Search,
            [Ask, Ask, Allow, Ask, Ask, Allow],
        ),
        (
            "Agent",
            Action::Agent {
                agent_type: "explore".into(),
            },
            [Allow; 6],
        ),
        (
            "mcp__github__get_issue",
            mcp("github", "get_issue", true),
            [Ask, Ask, Allow, Ask, Ask, Allow],
        ),
        (
            "mcp__github__create_issue",
            mcp("github", "create_issue", false),
            [Ask, Ask, Allow, Deny, Deny, Deny],
        ),
        ("Skill", Action::Skill { name: "pdf".into() }, [Allow; 6]),
        ("TodoWrite", Action::Other { read_only: true }, [Allow; 6]),
        (
            "ExitPlanMode",
            Action::Other { read_only: true },
            [Allow; 6],
        ),
        (
            "JobKill",
            Action::Other { read_only: false },
            [Ask, Ask, Allow, Deny, Deny, Deny],
        ),
    ];
    for (tool, action, expected) in cases {
        for ((planning, level), want) in COLUMNS.into_iter().zip(expected) {
            let decision = policy.decide(tool, &action, planning, level);
            assert_eq!(
                outcome(&decision),
                want,
                "{tool} {action:?} at {level:?}, planning {planning}: {decision:?}"
            );
        }
    }
}

#[test]
fn plan_mode_explains_how_to_leave() {
    let policy = policy(&["Bash", "Edit"], &[], &[]);
    for level in LEVELS {
        let decision = policy.decide("Edit", &write(&["/work/proj/a.rs"]), true, level);
        assert_eq!(
            decision,
            Decision::Deny {
                reason: PLAN_DENIAL.into()
            }
        );
        let command = plan_bash(&policy, "cargo build", level);
        assert_eq!(outcome(&command), Deny, "{level:?}");
        assert_eq!(outcome(&plan_bash(&policy, "git log", level)), Allow);
    }
}

#[test]
fn plan_with_bypass_researches_without_asking_and_never_writes() {
    let policy = policy(&[], &[], &[]);
    let bypass = PermissionMode::Bypass;
    let denied: Vec<(&str, Action)> = vec![
        ("Edit", write(&["/work/proj/src/lib.rs"])),
        ("Write", write(&["/tmp/notes.md"])),
        ("Bash", exec("cargo test")),
        ("Bash", exec("python3 scripts/gen.py")),
        ("Bash", exec("eval \"$CMD\"")),
        (
            "mcp__github__create_issue",
            mcp("github", "create_issue", false),
        ),
    ];
    for (tool, action) in denied {
        assert_eq!(
            policy.decide(tool, &action, true, bypass).reason(),
            PLAN_DENIAL,
            "{tool} {action:?}"
        );
    }
    let allowed: Vec<(&str, Action)> = vec![
        ("WebSearch", Action::Search),
        ("WebFetch", fetch("https://docs.rs/serde")),
        ("mcp__github__get_issue", mcp("github", "get_issue", true)),
        ("Read", read(&["/home/me/.cargo/config.toml"])),
        ("Bash", exec("cat /etc/hosts")),
    ];
    for (tool, action) in allowed {
        let decision = policy.decide(tool, &action, true, bypass);
        assert_eq!(decision.reason(), "bypass mode", "{tool} {action:?}");
    }
}

#[test]
fn bypass_still_honors_deny_rules() {
    let policy = policy(
        &[],
        &[],
        &[
            "Bash(rm:*)",
            "Read(~/.ssh/**)",
            "WebFetch",
            "mcp__prod",
            "JobKill",
        ],
    );
    let bypass = PermissionMode::Bypass;
    let denied: Vec<(&str, Action)> = vec![
        ("Bash", exec("rm -rf build")),
        ("Read", read(&["/home/me/.ssh/id_ed25519"])),
        ("WebFetch", fetch("https://example.com")),
        ("mcp__prod__drop", mcp("prod", "drop", false)),
        ("JobKill", Action::Other { read_only: false }),
    ];
    for (tool, action) in denied {
        for planning in [false, true] {
            assert_eq!(
                outcome(&policy.decide(tool, &action, planning, bypass)),
                Deny,
                "{tool}, planning {planning}"
            );
        }
    }
    assert_eq!(
        policy
            .decide("WebFetch", &fetch("https://a.io"), true, bypass)
            .reason(),
        "denied by rule WebFetch",
        "deny rules come before the planning floor"
    );
    assert_eq!(outcome(&bash(&policy, "cargo build", bypass)), Allow);
    assert_eq!(bash(&policy, "cargo build", bypass).reason(), "bypass mode");
}

#[test]
fn approval_cards_carry_suggestions_and_persistence() {
    let policy = policy(&[], &[], &[]);
    let decide =
        |tool: &str, action: &Action| policy.decide(tool, action, false, PermissionMode::Default);
    let outside_read = decide("Read", &read(&["/etc/hosts"]));
    assert_eq!(suggested(&outside_read), Some("Read(//etc/hosts)"));
    assert!(!persistable(&outside_read));
    let home_read = decide("Read", &read(&["/home/me/.cargo/config.toml"]));
    assert_eq!(suggested(&home_read), Some("Read(~/.cargo/config.toml)"));
    let edit = decide("Edit", &write(&["/work/proj/src/lib.rs"]));
    assert_eq!(suggested(&edit), Some("Edit"));
    assert!(persistable(&edit));
    assert_eq!(edit.reason(), "edits src/lib.rs");
    let outside_write = decide("Write", &write(&["/tmp/x[1].txt"]));
    assert_eq!(suggested(&outside_write), Some("Edit(//tmp/x[[]1[]].txt)"));
    assert!(!persistable(&outside_write));
    let protected = decide("Edit", &write(&["/work/proj/.git/hooks/pre-commit"]));
    assert_eq!(suggested(&protected), None);
    assert!(!persistable(&protected));
    let command = bash(&policy, "npm run test -- --watch", PermissionMode::Default);
    assert_eq!(suggested(&command), Some("Bash(npm run test:*)"));
    assert!(persistable(&command));
    let fetch = decide("WebFetch", &fetch("https://Docs.RS/serde"));
    assert_eq!(suggested(&fetch), Some("WebFetch(domain:docs.rs)"));
    let search = decide("WebSearch", &Action::Search);
    assert_eq!(suggested(&search), Some("WebSearch"));
    let tool = decide(
        "mcp__github__create_issue",
        &mcp("github", "create_issue", false),
    );
    assert_eq!(suggested(&tool), Some("mcp__github__create_issue"));
    let kill = decide("JobKill", &Action::Other { read_only: false });
    assert_eq!(suggested(&kill), Some("JobKill"));
}

#[test]
fn read_only_auto_allow_can_be_turned_off() {
    let settings = PolicyConfig {
        auto_allow_read_only_bash: false,
        ..PolicyConfig::default()
    };
    let (policy, _) = Policy::new(&settings, ctx());
    let status = bash(&policy, "git status", PermissionMode::Default);
    assert_eq!(outcome(&status), Ask);
    assert_eq!(status.reason(), "read-only commands need approval");
    assert_eq!(
        outcome(&bash(&policy, "ls", PermissionMode::AcceptEdits)),
        Ask
    );
    assert_eq!(
        outcome(&bash(&policy, "mkdir docs", PermissionMode::AcceptEdits)),
        Allow
    );
    assert_eq!(
        outcome(&plan_bash(&policy, "git status", PermissionMode::Default)),
        Ask
    );
    assert_eq!(
        outcome(&plan_bash(&policy, "git status", PermissionMode::Bypass)),
        Allow
    );
}
