//! Execute actions: shell rules per command, read-only, acceptEdits and
//! sandbox defaults, and the suggested "always allow" rule.

use std::path::{Path, PathBuf};

use z_engine_protocol::PermissionMode;

use super::sandbox;
use crate::engine::{Decision, Policy};
use crate::paths::resolve;
use crate::rules::Rule;
use crate::shell::{self, Location, Parsed, Segment, Word};

/// Everything the decision needs about one command line, computed once.
#[derive(Debug)]
pub(super) struct CommandFacts {
    command: String,
    parsed: Parsed,
    /// Every command the line may run, wrappers and substitutions included.
    candidates: Vec<Segment>,
    /// The candidate budget ran out; restrictive rules then match anyway.
    truncated: bool,
    read_only: bool,
}

impl CommandFacts {
    pub(super) fn new(command: &str) -> Self {
        let parsed = shell::parse(command);
        let mut candidates = Vec::new();
        let mut truncated = false;
        for segment in parsed.all_segments() {
            let (found, cut) = shell::executed_commands(segment);
            candidates.extend(found);
            truncated |= cut;
        }
        let read_only = shell::command_is_read_only(&parsed);
        Self {
            command: command.trim().to_string(),
            parsed,
            candidates,
            truncated,
            read_only,
        }
    }

    pub(super) fn read_only(&self) -> bool {
        self.read_only
    }
}

/// Deny and ask semantics: the verbatim line, any candidate command, or any
/// file a candidate names (for `Read(..)` / `Edit(..)` group rules).
pub(super) fn restricts(policy: &Policy, rule: &Rule, tool: &str, facts: &CommandFacts) -> bool {
    if rule.names_tool(tool) {
        return true;
    }
    if let Some(pattern) = rule.command_pattern() {
        return match pattern {
            None => true,
            Some(pattern) => {
                facts.truncated
                    || pattern.matches_line(&facts.command)
                    || facts.candidates.iter().any(|c| pattern.restricts(&c.words))
            }
        };
    }
    let reads = rule.operand_pattern(false).map(|pattern| (pattern, false));
    let Some((pattern, writes)) = reads.or(rule.operand_pattern(true).map(|p| (p, true))) else {
        return false;
    };
    facts.truncated
        || facts.candidates.iter().any(|candidate| {
            let operands = shell::operands(&candidate.words);
            let named: Vec<Word> = if writes {
                operands
                    .writes
                    .into_iter()
                    .chain(candidate.write_targets().cloned())
                    .collect()
            } else {
                operands.reads
            };
            named
                .iter()
                .filter_map(|word| best_guess(policy, word))
                .any(|path| pattern.matches(&path, &policy.ctx))
        })
}

pub(super) fn decide(
    policy: &Policy,
    tool: &str,
    facts: &CommandFacts,
    mode: PermissionMode,
) -> Decision {
    if let Some(rule) = allowing_rule(policy, tool, facts) {
        return Decision::allow(format!("allowed by rule {rule}"));
    }
    let top = &facts.parsed.segments;
    if facts.read_only
        && policy.auto_allow_read_only_bash
        && top.iter().all(|segment| reads_allowed(policy, segment))
    {
        return Decision::allow("read-only command");
    }
    if mode == PermissionMode::AcceptEdits
        && (policy.auto_allow_read_only_bash || !facts.read_only)
        && shell::is_common_fs_command(&facts.command, &policy.ctx.project_root)
    {
        return Decision::allow("acceptEdits mode allows filesystem commands inside the project");
    }
    if sandbox::allows(policy, &facts.parsed, &facts.candidates, facts.truncated) {
        return Decision::allow(sandbox::REASON);
    }
    Decision::ask(ask_reason(policy, facts), suggestion(policy, facts), true)
}

/// An allow rule for the tool or the whole line, or every top-level command
/// covered by a Bash rule or by being read-only. Prefix rules never cover
/// lines with substitutions, `eval`, or parse errors.
fn allowing_rule<'a>(policy: &'a Policy, tool: &str, facts: &CommandFacts) -> Option<&'a Rule> {
    let whole = policy.allow_rules().find(|rule| {
        rule.names_tool(tool)
            || match rule.command_pattern() {
                Some(None) => true,
                Some(Some(pattern)) => pattern.matches_line(&facts.command),
                None => false,
            }
    });
    if whole.is_some() {
        return whole;
    }
    let parsed = &facts.parsed;
    if !parsed.ok || parsed.dynamic || parsed.segments.is_empty() {
        return None;
    }
    let mut first = None;
    for segment in &parsed.segments {
        match coverage(policy, segment) {
            Coverage::Rule(rule) => {
                first.get_or_insert(rule);
            }
            Coverage::Harmless => {}
            Coverage::None => return None,
        }
    }
    first
}

enum Coverage<'a> {
    Rule(&'a Rule),
    /// Read-only (with auto-allow on), or no command at all (`fi`, `> log`).
    Harmless,
    None,
}

fn coverage<'a>(policy: &'a Policy, segment: &Segment) -> Coverage<'a> {
    let contained = redirects_contained(policy, segment);
    let words = segment.argv();
    if words.is_empty() {
        return if contained {
            Coverage::Harmless
        } else {
            Coverage::None
        };
    }
    if contained {
        let rule = policy.allow_rules().find(|rule| {
            rule.command_pattern()
                .flatten()
                .is_some_and(|pattern| pattern.allows(&words))
        });
        if let Some(rule) = rule {
            return Coverage::Rule(rule);
        }
    }
    if policy.auto_allow_read_only_bash
        && shell::segment_is_read_only(segment)
        && reads_allowed(policy, segment)
    {
        return Coverage::Harmless;
    }
    Coverage::None
}

/// Every file the command reads is provably inside the allowed directories
/// or covered by a `Read(..)` allow rule.
fn reads_allowed(policy: &Policy, segment: &Segment) -> bool {
    let home = policy.ctx.home.as_deref();
    shell::operands(segment.command())
        .reads
        .iter()
        .all(|word| match shell::locate(word, home) {
            Location::Relative { escapes, .. } => !escapes,
            Location::Absolute(path) => {
                policy.is_inside_allowed(&path) || read_rule_covers(policy, &path)
            }
            Location::Unknown => false,
        })
}

fn read_rule_covers(policy: &Policy, path: &Path) -> bool {
    policy.allow_rules().any(|rule| {
        rule.operand_pattern(false)
            .is_some_and(|pattern| pattern.matches(path, &policy.ctx))
    })
}

/// Redirect targets stay inside the allowed directories and off protected paths.
fn redirects_contained(policy: &Policy, segment: &Segment) -> bool {
    let root = &policy.ctx.project_root;
    segment.write_targets().all(
        |word| match shell::locate(word, policy.ctx.home.as_deref()) {
            Location::Relative {
                path,
                escapes: false,
            } => !policy.is_protected(&resolve(root, &path)),
            Location::Absolute(path) => {
                policy.is_inside_allowed(&path) && !policy.is_protected(&path)
            }
            _ => false,
        },
    )
}

/// Where a restrictive rule should look for an operand, assuming the shell
/// runs from the project root.
fn best_guess(policy: &Policy, word: &Word) -> Option<PathBuf> {
    match shell::locate(word, policy.ctx.home.as_deref()) {
        Location::Relative { path, .. } => Some(resolve(&policy.ctx.project_root, &path)),
        Location::Absolute(path) => Some(path),
        Location::Unknown => None,
    }
}

fn ask_reason(policy: &Policy, facts: &CommandFacts) -> &'static str {
    if !facts.parsed.ok {
        "the command could not be fully parsed"
    } else if facts.parsed.dynamic {
        "the command computes part of itself at run time"
    } else if facts.read_only && !policy.auto_allow_read_only_bash {
        "read-only commands need approval"
    } else if facts.read_only {
        "the command reads outside the project"
    } else {
        "the command needs approval"
    }
}

/// `Bash(<prefix>:*)` for the first uncovered command, or the exact line
/// when no prefix is safe (or a redirect would keep a prefix from applying).
fn suggestion(policy: &Policy, facts: &CommandFacts) -> Option<String> {
    let parsed = &facts.parsed;
    if !parsed.ok || parsed.dynamic {
        return None;
    }
    let segment = parsed
        .segments
        .iter()
        .find(|segment| matches!(coverage(policy, segment), Coverage::None))?;
    let prefix =
        shell::prefix_for(segment.command()).filter(|_| redirects_contained(policy, segment));
    let text = match prefix {
        Some(prefix) => format!("Bash({prefix}:*)"),
        None => {
            let command = facts.command.as_str();
            let expressible =
                command.len() <= 200 && !command.contains('\n') && !command.ends_with('*');
            if !expressible {
                return None;
            }
            format!("Bash({command})")
        }
    };
    Rule::parse(&text).ok().map(|rule| rule.to_string())
}
