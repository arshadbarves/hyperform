//! Sandboxed shell commands: with the OS sandbox confining writes, a
//! command that would otherwise ask is allowed when it parses statically
//! and every file it names for writing (redirect targets and operands of
//! `rm`, `cp`, `sed -i`, ...) stays inside the allowed directories and off
//! protected paths. The sandbox is what contains the rest.

use crate::engine::Policy;
use crate::paths::resolve;
use crate::shell::{self, Location, Segment, Word};

pub(super) const REASON: &str = "sandboxed";

/// `candidates` are every command the line may run, wrappers and
/// substitutions included; `truncated` means that list is incomplete.
pub(super) fn allows(
    policy: &Policy,
    parsed: &shell::Parsed,
    candidates: &[Segment],
    truncated: bool,
) -> bool {
    policy.sandbox_auto_allow
        && parsed.ok
        && !parsed.dynamic
        && !truncated
        && parsed
            .all_segments()
            .all(|segment| writes_contained(policy, segment))
        && candidates
            .iter()
            .all(|candidate| writes_contained(policy, candidate))
}

fn writes_contained(policy: &Policy, segment: &Segment) -> bool {
    let operands = shell::operands(segment.command()).writes;
    segment
        .write_targets()
        .chain(&operands)
        .all(|word| contained(policy, word))
}

fn contained(policy: &Policy, word: &Word) -> bool {
    let path = match shell::locate(word, policy.ctx.home.as_deref()) {
        Location::Relative {
            path,
            escapes: false,
        } => resolve(&policy.ctx.project_root, &path),
        Location::Absolute(path) if policy.is_inside_allowed(&path) => path,
        _ => return false,
    };
    !policy.is_protected(&path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use z_engine_protocol::PermissionMode;

    use crate::{Action, Decision, Policy, PolicyConfig, PolicyContext};

    fn policy(sandbox: bool) -> Policy {
        let config = PolicyConfig {
            sandbox_auto_allow: sandbox,
            ..PolicyConfig::default()
        };
        let ctx = PolicyContext {
            project_root: PathBuf::from("/work/proj"),
            additional_dirs: vec![PathBuf::from("/data/shared")],
            home: Some(PathBuf::from("/home/me")),
        };
        Policy::new(&config, ctx).0
    }

    fn decide(policy: &Policy, command: &str) -> Decision {
        let action = Action::Execute {
            command: command.to_string(),
        };
        policy.decide("Bash", &action, false, PermissionMode::Default)
    }

    #[test]
    fn contained_commands_are_allowed_only_when_sandboxed() {
        for command in [
            "cargo build",
            "npm install && npm test > out/log.txt",
            "rm -rf target/debug",
            "cp /data/shared/a.csv data/",
            "python3 scripts/gen.py",
        ] {
            assert!(
                matches!(decide(&policy(true), command), Decision::Allow { ref reason } if reason == "sandboxed"),
                "{command}"
            );
            assert!(
                matches!(decide(&policy(false), command), Decision::Ask { .. }),
                "{command}"
            );
        }
    }

    #[test]
    fn writes_outside_or_to_protected_paths_still_ask() {
        for command in [
            "rm -rf /etc/x",
            "echo x > ~/.bashrc",
            "cp a ../elsewhere/",
            "touch .git/hooks/pre-commit",
            "echo x > .z-engine/settings.toml",
            "echo $(curl x) > a",
            "eval \"$CMD\"",
            "rm \"$TARGET\"",
            "echo 'unterminated",
        ] {
            assert!(
                matches!(decide(&policy(true), command), Decision::Ask { .. }),
                "{command}"
            );
        }
    }
}
