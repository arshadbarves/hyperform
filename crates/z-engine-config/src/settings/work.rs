//! `[work]`: limits for work plans (`work_plans`): how large a task packet
//! may grow, which paths make a plan deep, how often a task may start, and
//! how parallel tasks (`work_parallel`) run and merge back.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Settings;

pub const WORK_PACKET_TOKENS: RangeInclusive<u32> = 500..=32_000;
pub const WORK_MAX_ATTEMPTS: RangeInclusive<u32> = 1..=20;
pub const WORK_MAX_PARALLEL: RangeInclusive<u32> = 1..=16;

/// Lockfiles, package manifests and generated output: files that many
/// tasks change as a side effect, so a task that names one runs alone.
pub const DEFAULT_SERIAL_PATHS: [&str; 22] = [
    "**/Cargo.lock",
    "**/Cargo.toml",
    "**/package-lock.json",
    "**/npm-shrinkwrap.json",
    "**/yarn.lock",
    "**/pnpm-lock.yaml",
    "**/bun.lock",
    "**/bun.lockb",
    "**/package.json",
    "**/poetry.lock",
    "**/uv.lock",
    "**/Pipfile.lock",
    "**/pyproject.toml",
    "**/go.mod",
    "**/go.sum",
    "**/Gemfile",
    "**/Gemfile.lock",
    "**/composer.json",
    "**/composer.lock",
    "**/generated/**",
    "**/__generated__/**",
    "crates/z-engine-gui/ui/src/lib/protocol/**",
];

/// How a parallel helper's finished worktree change comes back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum MergePolicy {
    /// Each change waits for you on an apply card.
    #[default]
    Ask,
    /// Changes apply one at a time when they merge cleanly; a conflict
    /// still stops and asks.
    Auto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct WorkSettings {
    /// Estimated tokens a task packet may take. The goal, invariants, the
    /// task and its criteria always fit; ranked context fills the rest.
    pub packet_tokens: u32,
    /// Globs relative to the project root; a plan whose task scopes touch
    /// one is suggested to plan deeply.
    pub sensitive_paths: Vec<String>,
    /// Times an agent may start one task; after that only you can retry it.
    pub max_attempts: u32,
    /// Tasks that may run at once in parallel helpers; never more than
    /// `agents.max_concurrent`.
    pub max_parallel: u32,
    /// `ask` or `auto`. In an untrusted workspace only your user settings
    /// can choose `auto`.
    pub merge: MergePolicy,
    /// Globs relative to the project root; a task whose scope names one
    /// never runs in parallel.
    pub serial_paths: Vec<String>,
}

impl Default for WorkSettings {
    fn default() -> Self {
        Self {
            packet_tokens: 4_000,
            sensitive_paths: Vec::new(),
            max_attempts: 3,
            max_parallel: 3,
            merge: MergePolicy::Ask,
            serial_paths: DEFAULT_SERIAL_PATHS.into_iter().map(String::from).collect(),
        }
    }
}

impl Settings {
    /// Parallel helpers allowed at once: `work.max_parallel`, capped by
    /// `agents.max_concurrent`, and at least one.
    pub fn max_parallel_tasks(&self) -> u32 {
        self.work
            .max_parallel
            .min(self.agents.max_concurrent)
            .max(1)
    }
}

/// Clamps the limits into range and drops blank globs, warning about each
/// change.
pub(super) fn normalize_work(work: &mut WorkSettings, w: &mut Vec<String>) {
    for (key, value, range) in [
        ("packet_tokens", &mut work.packet_tokens, WORK_PACKET_TOKENS),
        ("max_attempts", &mut work.max_attempts, WORK_MAX_ATTEMPTS),
        ("max_parallel", &mut work.max_parallel, WORK_MAX_PARALLEL),
    ] {
        let clamped = (*value).clamp(*range.start(), *range.end());
        if clamped != *value {
            w.push(format!(
                "work.{key} = {value} is out of range; using {clamped}"
            ));
            *value = clamped;
        }
    }
    for (key, globs) in [
        ("sensitive_paths", &mut work.sensitive_paths),
        ("serial_paths", &mut work.serial_paths),
    ] {
        let before = globs.len();
        globs.retain(|glob| !glob.trim().is_empty());
        if globs.len() != before {
            w.push(format!("work.{key} has empty entries; they are ignored"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_tables_keep_defaults_and_limits_are_clamped() {
        let parsed: WorkSettings = toml::from_str("max_attempts = 5\n").unwrap();
        assert_eq!(
            (parsed.packet_tokens, parsed.max_attempts),
            (4_000, 5),
            "{parsed:?}"
        );
        let mut work = WorkSettings {
            packet_tokens: 10,
            sensitive_paths: vec!["src/auth/**".into(), " ".into()],
            max_attempts: 0,
            ..WorkSettings::default()
        };
        let mut w = Vec::new();
        normalize_work(&mut work, &mut w);
        assert_eq!((work.packet_tokens, work.max_attempts), (500, 1));
        assert_eq!(work.sensitive_paths, vec!["src/auth/**".to_string()]);
        assert_eq!(w.len(), 3, "{w:?}");
        let mut defaults = WorkSettings::default();
        normalize_work(&mut defaults, &mut w);
        assert_eq!(w.len(), 3, "defaults need no adjustment");
    }

    #[test]
    fn parallel_keys_default_validate_and_parse() {
        let defaults = WorkSettings::default();
        assert_eq!(
            (defaults.max_parallel, defaults.merge),
            (3, MergePolicy::Ask)
        );
        assert!(defaults.serial_paths.iter().any(|p| p == "**/Cargo.lock"));
        assert!(
            defaults
                .serial_paths
                .iter()
                .any(|p| p == "crates/z-engine-gui/ui/src/lib/protocol/**")
        );
        let parsed: WorkSettings = toml::from_str(
            "max_parallel = 40\nmerge = \"auto\"\nserial_paths = [\"db/schema.sql\", \"\"]\n",
        )
        .unwrap();
        assert_eq!(parsed.merge, MergePolicy::Auto);
        let mut parsed = parsed;
        let mut w = Vec::new();
        normalize_work(&mut parsed, &mut w);
        assert_eq!(parsed.max_parallel, 16);
        assert_eq!(parsed.serial_paths, vec!["db/schema.sql".to_string()]);
        assert_eq!(
            w,
            [
                "work.max_parallel = 40 is out of range; using 16",
                "work.serial_paths has empty entries; they are ignored"
            ]
        );
        assert!(toml::from_str::<WorkSettings>("merge = \"sometimes\"\n").is_err());
    }

    #[test]
    fn helpers_never_exceed_the_agent_limit() {
        let mut settings = Settings::default();
        assert_eq!(settings.max_parallel_tasks(), 3);
        settings.agents.max_concurrent = 2;
        assert_eq!(settings.max_parallel_tasks(), 2);
        settings.agents.max_concurrent = 0;
        assert_eq!(settings.max_parallel_tasks(), 1);
    }
}
