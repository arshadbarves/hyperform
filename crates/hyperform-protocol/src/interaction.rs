//! Todos, structured questions, and plan review. Field names follow Claude
//! Code's `TodoWrite` / `AskUserQuestion` schemas so model input parses as-is.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::permission::PermissionMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TodoItem {
    pub content: String,
    /// Present-continuous label shown while in progress ("Running tests").
    #[serde(default)]
    pub active_form: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuestionOption {
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Question {
    pub question: String,
    /// Short chip label (max ~12 characters).
    #[serde(default)]
    pub header: String,
    pub options: Vec<QuestionOption>,
    #[serde(default)]
    pub multi_select: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuestionAnswer {
    /// The question text being answered.
    pub question: String,
    /// Selected option labels, or a single custom answer.
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum PlanDecision {
    /// Turn plan mode off and build, optionally with a user-edited plan.
    /// The permission level stays as it is unless `mode` overrides it.
    /// `handoff`: the user builds in a new chat, so this chat stops
    /// without implementing (the attached plan still becomes Active).
    Approve {
        #[serde(default)]
        mode: Option<PermissionMode>,
        #[serde(default)]
        edited_plan: Option<String>,
        #[serde(default)]
        handoff: bool,
    },
    /// Stay in plan mode and revise with this feedback.
    Revise { feedback: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_items_parse_claude_code_shape() {
        let item: TodoItem = serde_json::from_value(serde_json::json!({
            "content": "Run tests",
            "activeForm": "Running tests",
            "status": "in_progress"
        }))
        .unwrap();
        assert_eq!(item.status, TodoStatus::InProgress);
        assert_eq!(item.active_form, "Running tests");
    }

    #[test]
    fn plan_decisions_are_tagged() {
        let json = serde_json::to_value(PlanDecision::Approve {
            mode: Some(PermissionMode::AcceptEdits),
            edited_plan: None,
            handoff: true,
        })
        .unwrap();
        assert_eq!(json["type"], "approve");
        assert_eq!(json["mode"], "acceptEdits");
        assert!(json["editedPlan"].is_null());
        assert_eq!(json["handoff"], true);
    }

    #[test]
    fn build_keeps_the_level_unless_overridden() {
        let build: PlanDecision =
            serde_json::from_value(serde_json::json!({"type": "approve"})).unwrap();
        assert_eq!(
            build,
            PlanDecision::Approve {
                mode: None,
                edited_plan: None,
                handoff: false,
            }
        );
    }
}
