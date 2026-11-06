//! Packet selection: what a model gets about its task, ranked and cut to a
//! token budget. The goal, active invariants, the task with its criteria,
//! its blockers and scope are never dropped; dependencies, the latest
//! evidence per criterion and ledger entries fill the rest in that order.

mod rank;
mod size;

use hyperform_protocol::TaskId;
use hyperform_protocol::work::{
    PacketBlocker, PacketDependency, PacketEntry, PacketEvidence, PacketTaskLine, PlanItem,
    PlanItemKind, PlanItemStatus, PlanStatus, Task, TaskPacket, TaskStatus,
};

use crate::error::PacketError;
use crate::facts::Facts;
use crate::graph::{next_ready, satisfies};
use crate::held;
use crate::state::PlanState;

/// A token estimator, such as `hyperform_context::tokens::estimate_text`.
pub type Estimate<'a> = &'a dyn Fn(&str) -> u64;

/// The fallback estimator: four characters per token.
pub fn estimate_chars(text: &str) -> u64 {
    u64::try_from(text.chars().count())
        .unwrap_or(u64::MAX)
        .div_ceil(4)
}

pub fn select(
    state: &PlanState,
    task_id: &TaskId,
    budget: u64,
    facts: &Facts,
    estimate: Estimate,
) -> Result<TaskPacket, PacketError> {
    let task = state
        .task(task_id)
        .ok_or_else(|| PacketError::UnknownTask(task_id.clone()))?;
    let invariants = active_items(state, |item| item.kind == PlanItemKind::Invariant);
    let requirements = active_items(state, |item| task.requirements.contains(&item.id));
    let parent = task.parent.as_ref().and_then(|id| state.task(id)).map(line);
    let blockers = blockers(state, task);
    let mut fill = Fill::new(budget);
    fill.force(estimate(&size::core_text(
        &state.header.goal,
        &invariants,
        &requirements,
        task,
        parent.as_ref(),
        &blockers,
    )));
    let dependencies: Vec<PacketDependency> = task
        .depends_on
        .iter()
        .filter_map(|id| state.task(id))
        .filter_map(|dep| {
            let text = format!("{} {}", dep.title, dep.result.as_deref().unwrap_or(""));
            fill.take(estimate(&text)).then(|| PacketDependency {
                id: dep.id.clone(),
                title: dep.title.clone(),
                status: dep.status.clone(),
                result: dep.result.clone(),
            })
        })
        .collect();
    let evidence: Vec<PacketEvidence> = task
        .acceptance
        .iter()
        .filter_map(|criterion| Some((criterion, criterion.evidence.last()?)))
        .filter(|(_, evidence)| fill.take(estimate(&size::evidence_text(evidence))))
        .map(|(criterion, evidence)| PacketEvidence {
            criterion_id: criterion.id.clone(),
            evidence: evidence.clone(),
            stale: size::is_stale(evidence, facts),
        })
        .collect();
    let entries: Vec<PacketEntry> = rank::ranked(state, task)
        .into_iter()
        .filter_map(|(entry, reason)| {
            let tokens = estimate(&size::entry_text(entry));
            fill.take(tokens).then(|| PacketEntry {
                entry: entry.clone(),
                reason,
                tokens,
            })
        })
        .collect();
    Ok(TaskPacket {
        plan_id: state.header.plan_id.clone(),
        revision: state.revision(),
        goal: state.header.goal.clone(),
        invariants,
        requirements,
        task: task.clone(),
        parent,
        dependencies,
        entries,
        evidence,
        blockers,
        scope: task.scope.files.clone(),
        resume: task.resume.clone(),
        next_ready: next_ready(state, Some(task_id)).map(line),
        tokens: fill.used,
        budget,
        dropped: fill.dropped,
    })
}

/// Greedy budget: forced content always counts; optional content is taken
/// in order while it fits, and skipped (counted as dropped) when not.
struct Fill {
    budget: u64,
    used: u64,
    dropped: u32,
}

impl Fill {
    fn new(budget: u64) -> Self {
        Self {
            budget,
            used: 0,
            dropped: 0,
        }
    }

    fn force(&mut self, tokens: u64) {
        self.used = self.used.saturating_add(tokens);
    }

    fn take(&mut self, tokens: u64) -> bool {
        if self.used.saturating_add(tokens) <= self.budget {
            self.used += tokens;
            true
        } else {
            self.dropped += 1;
            false
        }
    }
}

fn active_items(state: &PlanState, keep: impl Fn(&PlanItem) -> bool) -> Vec<PlanItem> {
    state
        .items()
        .filter(|item| item.status == PlanItemStatus::Active && keep(item))
        .cloned()
        .collect()
}

fn blockers(state: &PlanState, task: &Task) -> Vec<PacketBlocker> {
    let mut out = Vec::new();
    if let TaskStatus::Blocked { reason } = &task.status {
        out.push(PacketBlocker::Blocked {
            reason: reason.clone(),
        });
    }
    for dep in task.depends_on.iter().filter_map(|id| state.task(id)) {
        if !satisfies(&dep.status) {
            out.push(PacketBlocker::Waiting { task: line(dep) });
        }
    }
    if task.held {
        out.push(PacketBlocker::Held);
    }
    let held_by_plan = if held::partial(state) {
        held::holds(state, &task.id)
    } else {
        state.header.status != PlanStatus::Active
    };
    if held_by_plan {
        out.push(PacketBlocker::Plan {
            status: state.header.status,
        });
    }
    out
}

fn line(task: &Task) -> PacketTaskLine {
    PacketTaskLine {
        id: task.id.clone(),
        title: task.title.clone(),
        status: task.status.clone(),
    }
}

#[cfg(test)]
mod tests;
