//! Read and Write actions: per-path rules and location defaults.

use std::path::{Path, PathBuf};

use z_engine_protocol::PermissionMode;

use crate::action::Action;
use crate::engine::{Decision, Policy};
use crate::paths::resolve;
use crate::rules::Rule;

/// Deny and ask semantics: the rule governs the action and matches any path.
pub(super) fn restricts(
    policy: &Policy,
    rule: &Rule,
    tool: &str,
    action: &Action,
    paths: &[PathBuf],
) -> bool {
    match rule.path_scope(tool, action) {
        None => false,
        Some(None) => true,
        Some(Some(pattern)) => targets(policy, action, paths)
            .iter()
            .any(|path| pattern.matches(path, &policy.ctx)),
    }
}

pub(super) fn decide(
    policy: &Policy,
    tool: &str,
    action: &Action,
    paths: &[PathBuf],
    mode: PermissionMode,
) -> Decision {
    let targets = targets(policy, action, paths);
    if let Some(rule) = allowing_rule(policy, tool, action, &targets) {
        return Decision::allow(format!("allowed by rule {rule}"));
    }
    let outside = targets.iter().find(|path| !policy.is_inside_allowed(path));
    match action {
        Action::Read { .. } => match outside {
            None => Decision::allow("reads inside the project"),
            Some(path) => Decision::ask(
                format!("reads outside the project: {}", path.display()),
                exact_rule(policy, "Read", path),
                false,
            ),
        },
        _ if targets.is_empty() => Decision::ask("the files to edit are unknown", None, false),
        _ => match (
            outside,
            targets.iter().find(|path| policy.is_protected(path)),
        ) {
            (Some(path), _) => Decision::ask(
                format!("writes outside the project: {}", path.display()),
                exact_rule(policy, "Edit", path),
                false,
            ),
            (None, Some(path)) => Decision::ask(
                format!("edits a protected path: {}", shown(policy, path)),
                None,
                false,
            ),
            (None, None) if mode == PermissionMode::AcceptEdits => {
                Decision::allow("acceptEdits mode allows edits inside the project")
            }
            (None, None) => Decision::ask(
                format!("edits {}", describe(policy, &targets)),
                Some("Edit".into()),
                true,
            ),
        },
    }
}

/// Any path outside the allowed directories (persisting is then refused).
pub(super) fn touches_outside(policy: &Policy, action: &Action) -> bool {
    match action {
        Action::Read { paths } | Action::Write { paths } => targets(policy, action, paths)
            .iter()
            .any(|path| !policy.is_inside_allowed(path)),
        _ => false,
    }
}

/// Normalized absolute targets; a read without paths reads the project root.
fn targets(policy: &Policy, action: &Action, paths: &[PathBuf]) -> Vec<PathBuf> {
    let root = &policy.ctx.project_root;
    if paths.is_empty() && matches!(action, Action::Read { .. }) {
        return vec![root.clone()];
    }
    paths.iter().map(|path| resolve(root, path)).collect()
}

/// Every path covered by some allow rule. Rules without a pattern cover only
/// allowed directories; protected paths are never covered.
fn allowing_rule<'a>(
    policy: &'a Policy,
    tool: &str,
    action: &Action,
    targets: &[PathBuf],
) -> Option<&'a Rule> {
    let writes = matches!(action, Action::Write { .. });
    let mut first = None;
    for path in targets {
        let inside = policy.is_inside_allowed(path);
        if writes && inside && policy.is_protected(path) {
            return None;
        }
        let rule = policy
            .allow_rules()
            .find(|rule| match rule.path_scope(tool, action) {
                None => false,
                Some(None) => inside,
                Some(Some(pattern)) => pattern.matches(path, &policy.ctx),
            })?;
        first.get_or_insert(rule);
    }
    first
}

/// `Read(//abs/path)` or `Read(~/path)` for exactly this path (a directory
/// rule also covers its contents).
fn exact_rule(policy: &Policy, tool: &str, path: &Path) -> Option<String> {
    let under_home = policy
        .ctx
        .home
        .as_deref()
        .and_then(|home| path.strip_prefix(home).ok())
        .filter(|rest| !rest.as_os_str().is_empty());
    let spec = match under_home {
        Some(rest) => format!("~/{}", slashed(rest)),
        None => format!("//{}", slashed(path).trim_start_matches('/')),
    };
    let text = format!("{tool}({})", globset::escape(&spec));
    Rule::parse(&text).ok().map(|rule| rule.to_string())
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A path shown relative to the project root when inside it.
fn shown(policy: &Policy, path: &Path) -> String {
    path.strip_prefix(&policy.ctx.project_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn describe(policy: &Policy, targets: &[PathBuf]) -> String {
    match targets {
        [only] => shown(policy, only),
        [first, rest @ ..] => format!("{} and {} more", shown(policy, first), rest.len()),
        [] => "nothing".into(),
    }
}
