//! The permission policy: settings, session context, and rule lists. The
//! decision itself lives in `crate::decide`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::PolicyError;
use crate::paths::{is_protected, normalize, resolve};
use crate::rules::{Rule, RuleKind};

/// Rule lists from settings, in the [`Rule::parse`] grammar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PolicyConfig {
    pub allow: Vec<String>,
    pub ask: Vec<String>,
    pub deny: Vec<String>,
    /// Run read-only shell commands that stay inside allowed directories
    /// without asking. On by default.
    pub auto_allow_read_only_bash: bool,
    /// Shell commands run in the OS sandbox: allow commands that would
    /// otherwise ask when every write they name stays inside the allowed
    /// directories. Deny and ask rules and plan mode still win. Set it only
    /// when the sandbox is enabled and available. Off by default.
    pub sandbox_auto_allow: bool,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            allow: Vec::new(),
            ask: Vec::new(),
            deny: Vec::new(),
            auto_allow_read_only_bash: true,
            sandbox_auto_allow: false,
        }
    }
}

/// Where the session runs. Paths should be absolute; the policy normalizes
/// them lexically and never resolves symlinks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyContext {
    pub project_root: PathBuf,
    /// Extra directories treated like the project for reads and edits.
    pub additional_dirs: Vec<PathBuf>,
    /// Expands `~/` in rules and shell operands; `None` leaves home
    /// patterns unmatched and `~` operands unresolved.
    pub home: Option<PathBuf>,
}

/// The policy's answer for one tool call. Reasons are short, human-readable
/// strings for the approval card or the model-facing refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow {
        reason: String,
    },
    /// Needs the user's approval. `suggested_rule` is offered for "always
    /// allow"; `can_persist` is false when the target is outside the
    /// allowed directories, so only this session may be granted.
    Ask {
        reason: String,
        suggested_rule: Option<String>,
        can_persist: bool,
    },
    Deny {
        reason: String,
    },
}

impl Decision {
    pub fn reason(&self) -> &str {
        match self {
            Self::Allow { reason } | Self::Ask { reason, .. } | Self::Deny { reason } => reason,
        }
    }

    pub(crate) fn allow(reason: impl Into<String>) -> Self {
        Self::Allow {
            reason: reason.into(),
        }
    }

    pub(crate) fn ask(
        reason: impl Into<String>,
        suggested_rule: Option<String>,
        can_persist: bool,
    ) -> Self {
        Self::Ask {
            reason: reason.into(),
            suggested_rule,
            can_persist,
        }
    }
}

/// Configured allow, ask, and deny rules plus rules granted during the
/// session, evaluated by [`Policy::decide`].
#[derive(Debug, Clone)]
pub struct Policy {
    pub(crate) ctx: PolicyContext,
    pub(crate) allow: Vec<Rule>,
    pub(crate) ask: Vec<Rule>,
    pub(crate) deny: Vec<Rule>,
    pub(crate) session: Vec<Rule>,
    pub(crate) auto_allow_read_only_bash: bool,
    pub(crate) sandbox_auto_allow: bool,
}

impl Policy {
    /// Builds the policy. Rules that do not parse are skipped and returned so
    /// the caller can show them; every other rule still applies.
    pub fn new(config: &PolicyConfig, ctx: PolicyContext) -> (Self, Vec<PolicyError>) {
        let mut errors = Vec::new();
        let mut parse = |texts: &[String]| {
            let mut rules: Vec<Rule> = Vec::new();
            for text in texts {
                match Rule::parse(text) {
                    Ok(rule) if !rules.contains(&rule) => rules.push(rule),
                    Ok(_) => {}
                    Err(error) => errors.push(error),
                }
            }
            rules
        };
        let allow = parse(&config.allow);
        let ask = parse(&config.ask);
        let deny = parse(&config.deny);
        let ctx = PolicyContext {
            project_root: normalize(&ctx.project_root),
            additional_dirs: ctx
                .additional_dirs
                .iter()
                .map(|dir| normalize(dir))
                .collect(),
            home: ctx.home.as_deref().map(normalize),
        };
        let policy = Self {
            ctx,
            allow,
            ask,
            deny,
            session: Vec::new(),
            auto_allow_read_only_bash: config.auto_allow_read_only_bash,
            sandbox_auto_allow: config.sandbox_auto_allow,
        };
        (policy, errors)
    }

    /// Grants an allow rule for the rest of the session (an "always allow"
    /// answer). Duplicates are ignored.
    pub fn add_session_rule(&mut self, rule: &str) -> Result<(), PolicyError> {
        let rule = Rule::parse(rule)?;
        if !self.session.contains(&rule) {
            self.session.push(rule);
        }
        Ok(())
    }

    /// Session-granted rules in canonical form.
    pub fn session_rules(&self) -> Vec<String> {
        self.session.iter().map(ToString::to_string).collect()
    }

    /// Configured rules of one kind in canonical form (session rules excluded).
    pub fn rules(&self, kind: RuleKind) -> Vec<String> {
        let rules = match kind {
            RuleKind::Allow => &self.allow,
            RuleKind::Ask => &self.ask,
            RuleKind::Deny => &self.deny,
        };
        rules.iter().map(ToString::to_string).collect()
    }

    /// Inside the project root or an additional directory, after lexical
    /// normalization (relative paths are taken from the project root).
    pub fn is_inside_allowed(&self, path: &Path) -> bool {
        let path = resolve(&self.ctx.project_root, path);
        self.roots().any(|root| path.starts_with(root))
    }

    /// The context with normalized paths.
    pub fn context(&self) -> &PolicyContext {
        &self.ctx
    }

    pub(crate) fn roots(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.ctx.project_root.as_path())
            .chain(self.ctx.additional_dirs.iter().map(PathBuf::as_path))
    }

    /// Configured and session allow rules.
    pub(crate) fn allow_rules(&self) -> impl Iterator<Item = &Rule> {
        self.allow.iter().chain(&self.session)
    }

    pub(crate) fn is_protected(&self, path: &Path) -> bool {
        is_protected(path, self.roots())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> PolicyContext {
        PolicyContext {
            project_root: PathBuf::from("/work/proj/./"),
            additional_dirs: vec![PathBuf::from("/data/shared/../shared")],
            home: Some(PathBuf::from("/home/me")),
        }
    }

    #[test]
    fn invalid_rules_are_reported_and_skipped() {
        let config = PolicyConfig {
            allow: vec![
                "Bash(cargo test:*)".into(),
                "Bash(".into(),
                "Bash(cargo test*)".into(),
            ],
            ask: vec!["WebSearch(x)".into()],
            deny: vec!["Read(~/.ssh/**)".into()],
            ..PolicyConfig::default()
        };
        let (policy, errors) = Policy::new(&config, ctx());
        assert_eq!(errors.len(), 2);
        assert_eq!(policy.rules(RuleKind::Allow), ["Bash(cargo test:*)"]);
        assert!(policy.rules(RuleKind::Ask).is_empty());
        assert_eq!(policy.rules(RuleKind::Deny), ["Read(~/.ssh/**)"]);
    }

    #[test]
    fn session_rules_parse_and_dedupe() {
        let (mut policy, _) = Policy::new(&PolicyConfig::default(), ctx());
        policy.add_session_rule("Bash(make build*)").unwrap();
        policy.add_session_rule("Bash(make build:*)").unwrap();
        policy.add_session_rule("Edit").unwrap();
        assert_eq!(policy.session_rules(), ["Bash(make build:*)", "Edit"]);
        assert!(policy.add_session_rule("Bash(").is_err());
        assert_eq!(policy.session_rules().len(), 2);
    }

    #[test]
    fn containment_is_lexical() {
        let (policy, _) = Policy::new(&PolicyConfig::default(), ctx());
        assert_eq!(policy.context().project_root, PathBuf::from("/work/proj"));
        assert!(policy.is_inside_allowed(Path::new("/work/proj/src/lib.rs")));
        assert!(policy.is_inside_allowed(Path::new("/work/proj")));
        assert!(policy.is_inside_allowed(Path::new("src/lib.rs")));
        assert!(policy.is_inside_allowed(Path::new("/data/shared/x.csv")));
        assert!(!policy.is_inside_allowed(Path::new("/work/proj/../proj2/x")));
        assert!(!policy.is_inside_allowed(Path::new("../outside.txt")));
        assert!(!policy.is_inside_allowed(Path::new("/work/project/x")));
        assert!(!policy.is_inside_allowed(Path::new("/etc/hosts")));
    }

    #[test]
    fn config_defaults_and_serde_shape() {
        assert!(PolicyConfig::default().auto_allow_read_only_bash);
        assert!(!PolicyConfig::default().sandbox_auto_allow);
        let config: PolicyConfig = serde_json::from_str(
            r#"{"allow":["Bash"],"autoAllowReadOnlyBash":false,"sandboxAutoAllow":true}"#,
        )
        .unwrap();
        assert_eq!(config.allow, ["Bash"]);
        assert!(!config.auto_allow_read_only_bash);
        assert!(config.sandbox_auto_allow);
        let empty: PolicyConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, PolicyConfig::default());
    }
}
