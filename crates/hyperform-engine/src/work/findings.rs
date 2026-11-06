//! Adding an audit finding to the plan (`addFinding`): the finding is
//! marked added and becomes a user-authored Pending task built from its
//! suggestion, linked to the requirements and files it is about, and the
//! main agent is told to work it into the plan.

use hyperform_context::work_finding_added;
use hyperform_protocol::work::{
    Actor, AuditFinding, AuditRef, AuditSeverity, AuditStatus, PlanItemKind, TaskScope, TaskStatus,
    WorkCause,
};
use hyperform_protocol::{AgentId, TaskId};
use hyperform_work::{PlanState, Proposal, Submission, TaskDraft};

use super::facts::instant;
use super::ids::Ids;
use super::lifecycle::report;
use super::plan::{PlanHandle, Submitted};
use crate::session::SessionCore;

/// Characters of a finding used as the new task's title.
const TITLE_CHARS: usize = 80;

pub(crate) fn add(core: &SessionCore, plan: &PlanHandle, finding_id: &str) {
    let mut added = None;
    let result = plan.submit(&instant(core), |state| {
        let finding = state
            .finding(finding_id)
            .ok_or_else(|| format!("there is no audit finding {finding_id}"))?;
        if finding.status == AuditStatus::Added {
            return Err(format!("finding {finding_id} is already in the plan"));
        }
        let task = Ids::new(state).task();
        let draft = task_for(state, finding, task.clone());
        added = Some((task, finding.clone()));
        let proposals = vec![
            Proposal::SetFindingStatus {
                finding_id: finding_id.to_string(),
                status: AuditStatus::Added,
            },
            Proposal::CreateTask(Box::new(draft)),
        ];
        Ok(Submission::new(Actor::User, WorkCause::Edit, proposals))
    });
    let accepted = matches!(result, Ok(Submitted::Accepted(_)));
    report(core, result, "The finding was not added to the plan");
    if let (true, Some((task, finding))) = (accepted, added) {
        core.reminders
            .push(&AgentId::main(), work_finding_added(&finding, &task));
    }
}

fn task_for(state: &PlanState, finding: &AuditFinding, id: TaskId) -> TaskDraft {
    let headline = finding.suggestion.as_deref().unwrap_or(&finding.text);
    let mut title: String = headline.trim().chars().take(TITLE_CHARS).collect();
    if headline.trim().chars().count() > TITLE_CHARS {
        title.push('…');
    }
    let mut requirements = Vec::new();
    let mut files = Vec::new();
    let mut tasks = Vec::new();
    for reference in &finding.refs {
        match reference {
            AuditRef::Item { item_id } => {
                let requirement = state
                    .item(item_id)
                    .is_some_and(|item| item.kind == PlanItemKind::Requirement);
                if requirement {
                    requirements.push(item_id.clone());
                }
            }
            AuditRef::File { path } => files.push(path.clone()),
            AuditRef::Task { task_id } | AuditRef::Criterion { task_id, .. } => {
                if !tasks.contains(task_id) {
                    tasks.push(task_id.clone());
                }
            }
        }
    }
    let mut description = format!(
        "From the plan audit ({} severity): {}",
        severity(finding.severity),
        finding.text.trim()
    );
    if let Some(suggestion) = &finding.suggestion {
        description.push_str(&format!("\nSuggested change: {}", suggestion.trim()));
    }
    if !tasks.is_empty() {
        let ids: Vec<&str> = tasks.iter().map(TaskId::as_str).collect();
        description.push_str(&format!("\nAbout tasks: {}", ids.join(", ")));
    }
    TaskDraft {
        id: Some(id),
        title,
        description,
        requirements,
        scope: TaskScope {
            files,
            symbols: Vec::new(),
        },
        status: Some(TaskStatus::Pending),
        ..TaskDraft::default()
    }
}

fn severity(severity: AuditSeverity) -> &'static str {
    match severity {
        AuditSeverity::Low => "low",
        AuditSeverity::Medium => "medium",
        AuditSeverity::High => "high",
    }
}
