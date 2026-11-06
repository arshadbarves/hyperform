//! Parallel eligibility: which ready tasks may start beside the running
//! (active) ones as parallel helpers, and why each other ready task may
//! not. A task that must run alone, declares no files or names a serial
//! path may still start when nothing else runs.

use hyperform_protocol::TaskId;
use hyperform_protocol::work::{
    ParallelBlock, ParallelExcluded, ParallelPolicy, ParallelSet, Task, TaskStatus,
};

use crate::dispatch::HELD_FOR_REPLAN;
use crate::facts::{Facts, Limits};
use crate::graph::{ordered, ready_or_replan_held, startable_tasks};
use crate::held::holds;
use crate::scope::{named, overlap};
use crate::state::PlanState;

/// Ready tasks that may start now, best first, cut to the free slots;
/// the tasks reconciliation findings hold are listed as `Replanning`.
pub fn eligible(state: &PlanState, facts: &Facts) -> ParallelSet {
    let candidates = ready_or_replan_held(state);
    let (held, ready): (Vec<TaskId>, Vec<TaskId>) =
        candidates.into_iter().partition(|id| holds(state, id));
    let mut set = choose(state, &ready, &facts.limits, gate(state, facts));
    set.excluded
        .extend(held.into_iter().map(|task_id| ParallelExcluded {
            task_id,
            block: ParallelBlock::Replanning,
        }));
    set
}

/// What approving parallel work would allow now: as `eligible`, ignoring
/// plan mode, the policy and the plan status (a draft's "N tasks can run
/// together").
pub fn preview(state: &PlanState, facts: &Facts) -> ParallelSet {
    choose(state, &startable_tasks(state), &facts.limits, None)
}

/// Why `task` cannot start beside the tasks running now, or `None`. Only
/// the policy and plan mode matter when nothing else runs.
pub fn beside_running(state: &PlanState, task: &Task, facts: &Facts) -> Option<ParallelBlock> {
    let others: Vec<&Task> = running(state).filter(|other| other.id != task.id).collect();
    if others.is_empty() {
        return None;
    }
    gate(state, facts)
        .or_else(|| own(task, &facts.limits))
        .or_else(|| {
            others
                .iter()
                .find_map(|other| beside(state, task, other, &facts.limits))
        })
        .or_else(|| slot(others.len(), facts.limits.max_parallel))
}

/// The block in words, for refusals and the model.
pub fn describe(block: &ParallelBlock) -> String {
    match block {
        ParallelBlock::Planning => "the session is in plan mode".into(),
        ParallelBlock::Replanning => HELD_FOR_REPLAN.into(),
        ParallelBlock::NotApproved { parallel } => format!(
            "parallel work is {} for this plan; only the user approves it",
            match parallel {
                ParallelPolicy::Off => "off",
                ParallelPolicy::Ask => "waiting for approval",
                ParallelPolicy::Approved => "approved",
            }
        ),
        ParallelBlock::RunAlone { task_id } => format!("task {task_id} must run alone"),
        ParallelBlock::UnknownScope { task_id } => {
            format!("task {task_id} declares no files, so overlap cannot be ruled out")
        }
        ParallelBlock::SerialPaths { task_id, patterns } => format!(
            "task {task_id} names serial paths ({})",
            patterns.join(", ")
        ),
        ParallelBlock::Ordered { task_id } => {
            format!("a dependency path links it to task {task_id}")
        }
        ParallelBlock::Overlap {
            task_id,
            files,
            symbols,
        } => format!(
            "its scope overlaps task {task_id} ({})",
            files
                .iter()
                .chain(symbols)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ParallelBlock::NoSlot { max } => format!("{max} tasks already run (work.max_parallel)"),
    }
}

fn gate(state: &PlanState, facts: &Facts) -> Option<ParallelBlock> {
    if facts.planning {
        Some(ParallelBlock::Planning)
    } else if state.header.parallel != ParallelPolicy::Approved {
        Some(ParallelBlock::NotApproved {
            parallel: state.header.parallel,
        })
    } else {
        None
    }
}

fn choose(
    state: &PlanState,
    candidates: &[TaskId],
    limits: &Limits,
    gate: Option<ParallelBlock>,
) -> ParallelSet {
    let running: Vec<&Task> = running(state).collect();
    let mut set = ParallelSet {
        running: running.iter().map(|task| task.id.clone()).collect(),
        max: limits.max_parallel.max(1),
        ..ParallelSet::default()
    };
    let mut chosen: Vec<&Task> = Vec::new();
    for task in candidates.iter().filter_map(|id| state.task(id)) {
        let company = running.len() + chosen.len();
        let block = gate
            .clone()
            .or_else(|| (company > 0).then(|| own(task, limits)).flatten())
            .or_else(|| {
                running
                    .iter()
                    .chain(&chosen)
                    .find_map(|other| beside(state, task, other, limits))
            })
            .or_else(|| slot(company, limits.max_parallel));
        match block {
            Some(block) => set.excluded.push(ParallelExcluded {
                task_id: task.id.clone(),
                block,
            }),
            None => {
                set.eligible.push(task.id.clone());
                chosen.push(task);
            }
        }
    }
    set
}

fn running(state: &PlanState) -> impl Iterator<Item = &Task> {
    state
        .tasks
        .values()
        .filter(|task| task.status == TaskStatus::Active)
}

/// What keeps `task` from running beside anything; a serial retry runs
/// alone too.
fn own(task: &Task, limits: &Limits) -> Option<ParallelBlock> {
    let task_id = task.id.clone();
    if task.run_alone || task.run.serial {
        return Some(ParallelBlock::RunAlone { task_id });
    }
    if task.scope.files.is_empty() {
        return Some(ParallelBlock::UnknownScope { task_id });
    }
    let patterns = named(&task.scope.files, &limits.serial_paths);
    (!patterns.is_empty()).then_some(ParallelBlock::SerialPaths { task_id, patterns })
}

/// What keeps `task` from running beside `other`, named after `other`.
fn beside(state: &PlanState, task: &Task, other: &Task, limits: &Limits) -> Option<ParallelBlock> {
    if let Some(block) = own(other, limits) {
        return Some(block);
    }
    let task_id = other.id.clone();
    if ordered(state, &task.id, &other.id) {
        return Some(ParallelBlock::Ordered { task_id });
    }
    let files = overlap(&task.scope.files, &other.scope.files);
    let symbols: Vec<String> = task
        .scope
        .symbols
        .iter()
        .filter(|symbol| other.scope.symbols.contains(symbol))
        .cloned()
        .collect();
    (!files.is_empty() || !symbols.is_empty()).then_some(ParallelBlock::Overlap {
        task_id,
        files,
        symbols,
    })
}

fn slot(taken: usize, max: u32) -> Option<ParallelBlock> {
    let max = max.max(1);
    (taken >= max as usize).then_some(ParallelBlock::NoSlot { max })
}

#[cfg(test)]
#[path = "parallel_tests.rs"]
mod tests;
