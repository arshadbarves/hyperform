//! Scope drift: a write by a task's helper (`Edit`, `Write`, `MultiEdit`,
//! or a shell redirect the policy can attribute) to a file outside the
//! task's declared scope. The gate asks first (an Allow becomes an Ask; a
//! denial stays), Bypass mode does not ask, and every such attempt is noted
//! in the plan as a kernel discovery on the task, before the write is
//! allowed or denied.

use std::path::{Path, PathBuf};

use hyperform_host::{relative_display, resolve};
use hyperform_policy::{Action, analyze};
use hyperform_protocol::work::{Actor, Discovery, EntryBody, WorkCause};
use hyperform_protocol::{AgentId, CallId, TaskId};
use hyperform_work::{EntryDraft, PlanState, Proposal, Submission, scope};

use super::facts::instant;
use super::ids::Ids;
use super::lifecycle::{attached, report};
use crate::session::SessionCore;

/// Writes outside the scope of a helper's task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Drift {
    pub task: TaskId,
    /// Relative to the helper's root.
    pub paths: Vec<String>,
}

impl Drift {
    /// Why the call needs approval.
    pub(crate) fn reason(&self) -> String {
        format!(
            "This helper works on task {} and would write outside the task's declared scope: {}",
            self.task,
            self.paths.join(", ")
        )
    }

    /// Notes each path once in the plan, as a kernel discovery on the task.
    pub(crate) fn record(&self, core: &SessionCore, tool: &str, call_id: &CallId) {
        let Some(plan) = attached(core) else {
            return;
        };
        let result = plan.submit(&instant(core), |state| {
            let mut ids = Ids::new(state);
            let mut proposals = Vec::new();
            for path in &self.paths {
                let text = format!(
                    "helper for {} tried to write outside its scope: {path}",
                    self.task
                );
                if noted(state, &text) {
                    continue;
                }
                let discovery = Discovery {
                    text,
                    source: Some(path.clone()),
                };
                proposals.push(Proposal::Record(Box::new(EntryDraft {
                    id: Some(ids.entry()),
                    tasks: vec![self.task.clone()],
                    files: vec![path.clone()],
                    ..EntryDraft::new(EntryBody::Discovery(discovery))
                })));
            }
            let cause = WorkCause::Tool {
                tool: tool.to_string(),
                call_id: Some(call_id.clone()),
            };
            Ok(Submission::new(Actor::Kernel, cause, proposals))
        });
        report(
            core,
            result,
            "The write outside the task's scope was not noted",
        );
    }
}

/// The writes of `action` outside the scope of the task `agent` was
/// dispatched for; `root` is the agent's tree.
pub(crate) fn outside_scope(
    core: &SessionCore,
    agent: &AgentId,
    root: &Path,
    action: &Action,
) -> Option<Drift> {
    if agent.is_main() {
        return None;
    }
    let written: Vec<PathBuf> = match action {
        Action::Write { paths } => paths.clone(),
        Action::Execute { command } => analyze(command).writes.iter().map(PathBuf::from).collect(),
        _ => return None,
    };
    if written.is_empty() {
        return None;
    }
    let task = core.with_state(|state| {
        let info = state.agents.iter().find(|info| info.agent_id == *agent)?;
        info.task_id.clone()
    })?;
    let plan = attached(core)?;
    let paths: Vec<String> = plan.read(|state| {
        let bound = state.task(&task)?;
        let outside = written
            .iter()
            .map(|path| relative(root, path))
            .filter(|path| scope::outside(bound, path));
        Some(outside.collect())
    })?;
    (!paths.is_empty()).then_some(Drift { task, paths })
}

fn noted(state: &PlanState, text: &str) -> bool {
    state.entries.values().any(|entry| match &entry.body {
        EntryBody::Discovery(discovery) => discovery.text == text,
        _ => false,
    })
}

/// `path` relative to `root` with `.` and `..` resolved, also when only
/// one of them goes through a symlink (macOS `/var` is `/private/var`); a
/// path outside `root` stays absolute.
fn relative(root: &Path, path: &Path) -> String {
    relative_display(root, &resolve(root, path)).replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_relative_to_the_helper_root() {
        let root = Path::new("/wt/agt_1");
        assert_eq!(relative(root, Path::new("src/./a.rs")), "src/a.rs");
        assert_eq!(
            relative(root, Path::new("/wt/agt_1/docs/x.md")),
            "docs/x.md"
        );
        assert_eq!(relative(root, Path::new("src/../Cargo.toml")), "Cargo.toml");
        assert_eq!(relative(root, Path::new("/etc/hosts")), "/etc/hosts");
    }

    #[cfg(unix)]
    #[test]
    fn a_root_reached_through_a_symlink_still_relativizes() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("src")).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let real = std::fs::canonicalize(&real).unwrap();
        assert_eq!(relative(&link, &real.join("src/a.rs")), "src/a.rs");
        assert_eq!(relative(&real, &link.join("src/new.rs")), "src/new.rs");
        assert_eq!(relative(&real, &link.join("docs/x.md")), "docs/x.md");
    }

    #[test]
    fn the_reason_names_the_task_and_paths() {
        let drift = Drift {
            task: TaskId::from("t2"),
            paths: vec!["docs/x.md".into()],
        };
        assert!(drift.reason().contains("task t2"));
        assert!(drift.reason().ends_with("docs/x.md"));
    }
}
