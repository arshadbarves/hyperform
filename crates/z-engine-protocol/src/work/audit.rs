//! Plan audit findings: coverage gaps the kernel finds deterministically,
//! and what the read-only audit agent adds as a model review. A finding is
//! advice; adding it to the plan sends it to the agent as a proposal.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{EntryId, TaskId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AuditKind {
    /// An active requirement no planned task with criteria serves.
    UncoveredRequirement,
    /// A check criterion of a kind the project has no check for.
    CriterionWithoutMethod,
    /// An open task with no acceptance criteria.
    TaskWithoutCriteria,
    /// A task touching a risky path (persistence, protocol, sensitive
    /// paths) without a test or migration criterion.
    RiskyPathWithoutTest,
    /// Risks noted without a rollback or mitigation.
    RiskWithoutMitigation,
    /// Unrelated open tasks whose scopes overlap.
    Overlap,
    /// Open tasks with the same title.
    Duplicate,
    /// Added by the audit agent.
    ModelReview,
}

impl AuditKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::UncoveredRequirement => "uncoveredRequirement",
            Self::CriterionWithoutMethod => "criterionWithoutMethod",
            Self::TaskWithoutCriteria => "taskWithoutCriteria",
            Self::RiskyPathWithoutTest => "riskyPathWithoutTest",
            Self::RiskWithoutMitigation => "riskWithoutMitigation",
            Self::Overlap => "overlap",
            Self::Duplicate => "duplicate",
            Self::ModelReview => "modelReview",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AuditSeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AuditStatus {
    #[default]
    Open,
    /// The user added it to the plan: the agent was asked to address it.
    Added,
    Dismissed,
    /// A later audit no longer finds it.
    Resolved,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AuditSource {
    #[default]
    Deterministic,
    AuditAgent,
}

/// What a finding is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum AuditRef {
    Task {
        task_id: TaskId,
    },
    Criterion {
        task_id: TaskId,
        criterion_id: String,
    },
    /// A requirement or invariant.
    Item {
        item_id: EntryId,
    },
    File {
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditFinding {
    /// Stable for deterministic findings (`taskWithoutCriteria:tsk_1`), so
    /// a dismissal sticks across audits; `review-<n>` for the audit agent.
    pub id: String,
    pub kind: AuditKind,
    pub severity: AuditSeverity,
    pub text: String,
    #[serde(default)]
    pub refs: Vec<AuditRef>,
    /// A suggested fix, phrased as a plan change.
    #[serde(default)]
    pub suggestion: Option<String>,
    #[serde(default)]
    pub status: AuditStatus,
    #[serde(default)]
    pub source: AuditSource,
    #[ts(type = "number")]
    #[serde(default)]
    pub created_revision: u64,
    /// The revision that last changed it.
    #[ts(type = "number")]
    #[serde(default)]
    pub revision: u64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn kind_labels_match_serde() {
        let kinds = [
            AuditKind::UncoveredRequirement,
            AuditKind::CriterionWithoutMethod,
            AuditKind::TaskWithoutCriteria,
            AuditKind::RiskyPathWithoutTest,
            AuditKind::RiskWithoutMitigation,
            AuditKind::Overlap,
            AuditKind::Duplicate,
            AuditKind::ModelReview,
        ];
        for kind in kinds {
            assert_eq!(serde_json::to_value(kind).unwrap(), kind.label());
        }
    }

    #[test]
    fn findings_default_to_open_deterministic() {
        let finding: AuditFinding = serde_json::from_value(json!({
            "id": "taskWithoutCriteria:tsk_a", "kind": "taskWithoutCriteria",
            "severity": "medium", "text": "Task A has no acceptance criteria",
            "refs": [{"type": "task", "taskId": "tsk_a"}]
        }))
        .unwrap();
        assert_eq!(
            (finding.status, finding.source),
            (AuditStatus::Open, AuditSource::Deterministic)
        );
        assert_eq!(
            finding.refs,
            [AuditRef::Task {
                task_id: TaskId::from("tsk_a")
            }]
        );
        let json = serde_json::to_value(&finding).unwrap();
        assert_eq!(json["createdRevision"], 0);
        assert_eq!(json["suggestion"], serde_json::Value::Null);
    }
}
