//! A task's run in a parallel helper: where it was dispatched, how its
//! worktree change comes back into the project, and what the helper
//! reported. Kept on the task as `run`; a new dispatch starts a new run.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AgentId, SessionId};
use crate::work::report::TaskReport;

/// Set when `Agent { task_id }` dispatches the task to a helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskDispatch {
    pub session_id: SessionId,
    pub agent_id: AgentId,
    /// The helper's worktree; `None` when it works in the project tree.
    #[serde(default)]
    pub worktree: Option<String>,
    /// Which start of the task this is (1-based); the kernel sets it.
    #[serde(default)]
    pub attempt: u32,
    #[ts(type = "number")]
    #[serde(default)]
    pub at: u64,
}

/// Bringing a helper's worktree change back into the project tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Integration {
    /// The helper finished; its change waits to be applied (`merge = "ask"`).
    PendingApply { worktree: String },
    /// Applied cleanly; the task verifies on the project tree.
    Applied {
        #[serde(default)]
        files: Vec<String>,
    },
    /// Applying conflicted: the project tree is untouched and the
    /// worktree is kept for a serial retry.
    Conflict {
        paths: Vec<String>,
        worktree: String,
    },
    /// The change was thrown away; the task is blocked until it is retried.
    Discarded,
}

/// The current run of a task. Empty until the task is dispatched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskRun {
    #[serde(default)]
    pub dispatch: Option<TaskDispatch>,
    #[serde(default)]
    pub integration: Option<Integration>,
    #[serde(default)]
    pub report: Option<TaskReport>,
    /// Starts that no longer count against `max_attempts`: the user
    /// retried the task after them.
    #[serde(default)]
    pub forgiven_attempts: u32,
    /// Retried serially (after a merge conflict): the task runs alone
    /// until it is retried again without this.
    #[serde(default)]
    pub serial: bool,
}

impl TaskRun {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::work::{ReportSource, Task};

    #[test]
    fn tasks_logged_before_runs_load_with_an_empty_run() {
        let task: Task = serde_json::from_value(json!({
            "id": "tsk_a", "parent": null, "title": "A", "kind": "implement",
            "priority": "normal", "order": 0, "status": {"type": "pending"},
            "held": false, "runAlone": false, "dependsOn": [], "requirements": [],
            "scope": {}, "notes": {}, "acceptance": [], "steps": [], "owner": null,
            "result": null, "resume": null, "attempts": 2, "createdAt": 1,
            "updatedAt": 1, "createdRevision": 1, "revision": 1
        }))
        .unwrap();
        assert!(task.run.is_empty());
    }

    #[test]
    fn runs_serialize_camel_case() {
        let run = TaskRun {
            dispatch: Some(TaskDispatch {
                session_id: SessionId::from("S"),
                agent_id: AgentId::from("agt_1"),
                worktree: Some("/wt/a".into()),
                attempt: 2,
                at: 5,
            }),
            integration: Some(Integration::Conflict {
                paths: vec!["src/a.rs".into()],
                worktree: "/wt/a".into(),
            }),
            report: Some(TaskReport {
                source: ReportSource::Unreported,
                ..TaskReport::default()
            }),
            forgiven_attempts: 1,
            serial: true,
        };
        let json = serde_json::to_value(&run).unwrap();
        assert_eq!(json["dispatch"]["agentId"], "agt_1");
        assert_eq!(
            json["integration"],
            json!({"type": "conflict", "paths": ["src/a.rs"], "worktree": "/wt/a"})
        );
        assert_eq!(json["report"]["source"], "unreported");
        assert_eq!(json["report"]["diffKnown"], false);
        assert_eq!(json["forgivenAttempts"], 1);
        assert_eq!(json["serial"], true);
        assert_eq!(serde_json::from_value::<TaskRun>(json).unwrap(), run);
        let pending: Integration =
            serde_json::from_value(json!({"type": "pendingApply", "worktree": "/wt"})).unwrap();
        assert_eq!(
            pending,
            Integration::PendingApply {
                worktree: "/wt".into()
            }
        );
    }
}
