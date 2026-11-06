//! The environment section of the system prompt.

use z_engine_prompts::reminders::{ENVIRONMENT, GIT_SNAPSHOT};

use crate::template::render_template;

/// Shown as the status of a repository without local changes.
const CLEAN_STATUS: &str = "(clean)";
/// Shown as the history of a repository without commits.
const NO_COMMITS: &str = "(none)";

/// Git state captured once at session start; rendered as a snapshot so the
/// environment section stays byte-stable for prompt caching.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitInfo {
    pub branch: String,
    /// `git status --short` output; empty for a clean working tree.
    pub status_short: String,
    /// One line per commit, newest first (`abc1234 Fix parser`).
    pub recent_commits: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    pub cwd: String,
    pub project_root: String,
    pub platform: String,
    pub shell: String,
    /// `YYYY-MM-DD`, usually [`today`].
    pub date: String,
    pub model: String,
    /// `None` when the project is not a git repository.
    pub git: Option<GitInfo>,
    pub additional_dirs: Vec<String>,
}

/// Today's local date as `YYYY-MM-DD`.
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Renders [`ENVIRONMENT`], embedding [`GIT_SNAPSHOT`] for repositories.
/// Empty optional values (shell, additional directories, git) drop their
/// lines.
pub fn render_environment(env: &Environment) -> String {
    let additional_dirs = env.additional_dirs.join(", ");
    let git = env.git.as_ref().map(render_git).unwrap_or_default();
    let is_git_repo = if env.git.is_some() { "Yes" } else { "No" };
    render_template(
        ENVIRONMENT,
        &[
            ("cwd", &env.cwd),
            ("project_root", &env.project_root),
            ("additional_dirs", &additional_dirs),
            ("is_git_repo", is_git_repo),
            ("platform", &env.platform),
            ("shell", &env.shell),
            ("date", &env.date),
            ("model", &env.model),
            ("git", &git),
        ],
    )
    .trim_end()
    .to_string()
}

fn render_git(git: &GitInfo) -> String {
    // Leading spaces are significant in `git status --short` columns.
    let status = git.status_short.trim_end();
    let status = if status.trim().is_empty() {
        CLEAN_STATUS
    } else {
        status
    };
    let commits = git.recent_commits.join("\n");
    let commits = if commits.trim().is_empty() {
        NO_COMMITS
    } else {
        &commits
    };
    render_template(
        GIT_SNAPSHOT,
        &[
            ("branch", git.branch.trim()),
            ("status", status),
            ("commits", commits),
        ],
    )
    .trim_end()
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(git: Option<GitInfo>) -> Environment {
        Environment {
            cwd: "/work/app/src".into(),
            project_root: "/work/app".into(),
            platform: "darwin".into(),
            shell: "zsh".into(),
            date: "2026-09-23".into(),
            model: "claude-sonnet".into(),
            git,
            additional_dirs: Vec::new(),
        }
    }

    #[test]
    fn renders_every_field_and_the_git_snapshot() {
        let mut env = environment(Some(GitInfo {
            branch: "v2".into(),
            status_short: " M src/lib.rs\n?? notes.md\n".into(),
            recent_commits: vec!["abc1234 Add parser".into(), "def5678 Init".into()],
        }));
        env.additional_dirs = vec!["/work/shared".into(), "/work/docs".into()];
        let out = render_environment(&env);
        for expected in [
            "Working directory: /work/app/src",
            "Project root: /work/app",
            "Additional working directories: /work/shared, /work/docs",
            "Is directory a git repo: Yes",
            "Platform: darwin",
            "Shell: zsh",
            "Today's date: 2026-09-23",
            "Model: claude-sonnet",
            "Current branch: v2",
            " M src/lib.rs\n?? notes.md",
            "abc1234 Add parser\ndef5678 Init",
        ] {
            assert!(out.contains(expected), "missing {expected:?} in:\n{out}");
        }
        assert!(!out.contains("{{"), "{out}");
    }

    #[test]
    fn optional_lines_disappear_without_values() {
        let mut env = environment(None);
        env.shell.clear();
        let out = render_environment(&env);
        assert!(out.contains("Is directory a git repo: No"));
        assert!(!out.contains("Additional working directories"));
        assert!(!out.contains("Shell:"));
        assert!(!out.contains("Current branch"));
        assert!(!out.contains("{{"), "{out}");
        assert!(out.ends_with("</env>"), "{out}");
    }

    #[test]
    fn clean_trees_and_empty_histories_are_labelled() {
        let out = render_environment(&environment(Some(GitInfo {
            branch: "main".into(),
            ..GitInfo::default()
        })));
        assert!(out.contains("Status:\n(clean)"), "{out}");
        assert!(out.contains("Recent commits:\n(none)"), "{out}");
    }

    #[test]
    fn today_is_an_iso_date() {
        let date = today();
        assert!(
            chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_ok(),
            "{date}"
        );
    }
}
