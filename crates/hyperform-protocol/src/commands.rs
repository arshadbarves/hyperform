//! GUI -> engine directives for one session.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AgentId, JobId, MessageId, RequestId};
use crate::interaction::{PlanDecision, QuestionAnswer};
use crate::permission::{ApprovalDecision, PermissionMode};
use crate::session::{Effort, RewindScope};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Attachment {
    /// A pasted or picked image (base64 without a data-URL prefix).
    Image { media_type: String, data: String },
    /// A file whose contents are attached to the message.
    File { path: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Command {
    /// Start a turn (or queue it as steering when a turn is running).
    Submit {
        text: String,
        #[serde(default)]
        attachments: Vec<Attachment>,
    },
    /// Inject a message at the running turn's next round boundary.
    Steer {
        text: String,
    },
    /// Stop the current round; if `text` is set, send it immediately after.
    Interrupt {
        #[serde(default)]
        text: Option<String>,
    },
    /// Cancel the foreground turn (background jobs keep running).
    Cancel,
    ResolveApproval {
        request_id: RequestId,
        decision: ApprovalDecision,
    },
    /// `answers: None` dismisses the question.
    AnswerQuestion {
        request_id: RequestId,
        #[serde(default)]
        answers: Option<Vec<QuestionAnswer>>,
    },
    ResolvePlan {
        request_id: RequestId,
        decision: PlanDecision,
    },
    /// Set the permission level; planning stays as it is.
    SetMode {
        mode: PermissionMode,
    },
    /// Turn plan mode on or off; the permission level stays as it is.
    SetPlanning {
        planning: bool,
    },
    SetModel {
        model: String,
    },
    SetEffort {
        #[serde(default)]
        effort: Option<Effort>,
    },
    Compact {
        #[serde(default)]
        instructions: Option<String>,
    },
    Rewind {
        message_id: MessageId,
        scope: RewindScope,
    },
    KillJob {
        job_id: JobId,
    },
    ApplyAgentChanges {
        agent_id: AgentId,
    },
    DiscardAgentChanges {
        agent_id: AgentId,
    },
    /// A slash command, e.g. `name: "review", args: "src/"`.
    RunCommand {
        name: String,
        #[serde(default)]
        args: String,
    },
    /// `!cmd`: run a shell command without the model.
    Shell {
        command: String,
    },
    /// Replace the queued steering messages.
    EditQueue {
        queued: Vec<String>,
    },
    ReloadExtensions,
    /// Answer to `TrustRequired`: `true` trusts the project root and
    /// reloads its settings; `false` dismisses the request for this session.
    TrustWorkspace {
        trusted: bool,
    },
    /// The user acted on (`accepted`) or dismissed a `Suggested` card.
    ResolveSuggestion {
        suggestion_id: String,
        accepted: bool,
    },
    /// Bring back the exchanges a task view set aside, until the next
    /// task boundary.
    IncludeFullHistory,
    /// A user edit of a work plan from the Plan tab.
    EditWork {
        plan_id: crate::ids::PlanId,
        edit: crate::work::WorkEdit,
    },
    /// Attach a work plan of this project to the session; it is logged and
    /// reattached when the session reopens.
    AttachPlan {
        plan_id: crate::ids::PlanId,
    },
    /// Detach the session's work plan; the plan itself is kept.
    DetachPlan,
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_json_deserializes() {
        let cmd: Command = serde_json::from_value(serde_json::json!({
            "type": "resolveApproval",
            "requestId": "req_1",
            "decision": {"type": "allowSession", "rule": "Bash(cargo test:*)"}
        }))
        .unwrap();
        assert!(matches!(cmd, Command::ResolveApproval { .. }));

        let submit: Command =
            serde_json::from_value(serde_json::json!({"type": "submit", "text": "hi"})).unwrap();
        assert_eq!(
            submit,
            Command::Submit {
                text: "hi".into(),
                attachments: vec![]
            }
        );
    }

    #[test]
    fn set_planning_is_its_own_command() {
        let cmd: Command =
            serde_json::from_value(serde_json::json!({"type": "setPlanning", "planning": true}))
                .unwrap();
        assert_eq!(cmd, Command::SetPlanning { planning: true });
        let old: Result<Command, _> =
            serde_json::from_value(serde_json::json!({"type": "setMode", "mode": "plan"}));
        assert!(old.is_err(), "plan is no longer a permission level");
    }

    #[test]
    fn trust_workspace_uses_camel_case() {
        let cmd: Command =
            serde_json::from_value(serde_json::json!({"type": "trustWorkspace", "trusted": true}))
                .unwrap();
        assert_eq!(cmd, Command::TrustWorkspace { trusted: true });
        let json = serde_json::to_value(Command::TrustWorkspace { trusted: false }).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "trustWorkspace", "trusted": false})
        );
    }

    #[test]
    fn plan_attachment_commands_use_camel_case() {
        let plan_id = crate::ids::PlanId::new();
        let json = serde_json::to_value(Command::AttachPlan {
            plan_id: plan_id.clone(),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "attachPlan", "planId": plan_id.to_string()})
        );
        let detach: Command =
            serde_json::from_value(serde_json::json!({"type": "detachPlan"})).unwrap();
        assert_eq!(detach, Command::DetachPlan);
    }
}
