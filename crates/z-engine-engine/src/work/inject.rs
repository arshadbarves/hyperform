//! What the main agent is told about its plan without asking: a short pin
//! at each turn start, and the full packet of its focus task when the task
//! changes, after compaction and on attach; a task-bound helper gets its
//! packet again after its own compaction. Each packet is announced with
//! `taskPacketSent`.

use std::path::{Path, PathBuf};

use z_engine_context::{WorkPin, estimate_text, work_packet, work_pin};
use z_engine_protocol::work::TaskStatus;
use z_engine_protocol::{AgentId, Event, TaskId};
use z_engine_work::{Facts, graph, select};

use super::facts::{agent, current, focus, owned_active};
use super::lifecycle::attached;
use super::plan::PlanHandle;
use crate::run::RunContext;
use crate::session::SessionCore;

/// The turn-start pin while a plan is attached.
pub(crate) fn pin(core: &SessionCore) -> Option<String> {
    let plan = attached(core)?;
    let actor = agent(core, &AgentId::main());
    Some(plan.read(|state| {
        let current = owned_active(state, &actor);
        let next = graph::next_ready(state, current.map(|task| &task.id));
        work_pin(&WorkPin {
            header: &state.header,
            current,
            next,
        })
    }))
}

/// The main agent's focus packet, when one is due.
pub(crate) async fn due_packet(ctx: &RunContext) -> Option<String> {
    if !ctx.spec.is_main() {
        return None;
    }
    let core = &ctx.core;
    let plan = attached(core)?;
    let actor = agent(core, &ctx.spec.agent_id);
    let (task, revision) =
        plan.read(|state| focus(state, &actor).map(|task| (task.id.clone(), state.revision())))?;
    let reason = core.work.take_due(&task, revision)?;
    let facts = current(core, &ctx.spec.root).await;
    render(core, &plan, &AgentId::main(), &task, &facts, reason)
}

/// The focus packet right now (after compaction), counted as sent.
pub(crate) async fn packet_now(
    core: &SessionCore,
    root: &Path,
    reason: &'static str,
) -> Option<String> {
    let plan = attached(core)?;
    let actor = agent(core, &AgentId::main());
    let (task, revision) =
        plan.read(|state| focus(state, &actor).map(|task| (task.id.clone(), state.revision())))?;
    let facts = current(core, root).await;
    let text = render(core, &plan, &AgentId::main(), &task, &facts, reason)?;
    core.work.mark_sent(&task, revision);
    Some(text)
}

/// A task-bound helper's packet again, after its own compaction.
pub(crate) async fn helper_packet(core: &SessionCore, helper: &AgentId) -> Option<String> {
    if helper.is_main() {
        return None;
    }
    let plan = attached(core)?;
    let (task, root) = core.with_state(|state| {
        let info = state.agents.iter().find(|info| info.agent_id == *helper)?;
        let root = info
            .worktree
            .as_ref()
            .map(|worktree| PathBuf::from(&worktree.path));
        Some((info.task_id.clone()?, root))
    })?;
    let facts = current(core, root.as_deref().unwrap_or(&core.root)).await;
    let owned = plan.read(|state| {
        state.task(&task).is_some_and(|task| {
            task.status == TaskStatus::Active
                && task
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.agent_id == *helper)
        })
    });
    owned.then_some(())?;
    render(core, &plan, helper, &task, &facts, "compaction")
}

fn render(
    core: &SessionCore,
    plan: &PlanHandle,
    agent_id: &AgentId,
    task: &TaskId,
    facts: &Facts,
    reason: &'static str,
) -> Option<String> {
    let budget = u64::from(core.settings().settings.work.packet_tokens);
    let packet = plan
        .read(|state| select(state, task, budget, facts, &estimate_text))
        .map_err(|error| tracing::warn!(%error, "task packet not built"))
        .ok()?;
    core.events.emit(Event::TaskPacketSent {
        agent_id: agent_id.clone(),
        task_id: task.clone(),
        tokens: packet.tokens,
        reason: reason.to_string(),
    });
    Some(work_packet(&packet))
}

/// Literal scope files of the main agent's focus task and of every active
/// task, for the repository map's ranking.
pub(crate) fn focus_files(core: &SessionCore) -> Vec<String> {
    let Some(plan) = attached(core) else {
        return Vec::new();
    };
    let actor = agent(core, &AgentId::main());
    plan.read(|state| {
        let focused = focus(state, &actor).into_iter();
        let active = state
            .tasks
            .values()
            .filter(|task| task.status == TaskStatus::Active);
        let mut files: Vec<String> = focused
            .chain(active)
            .flat_map(|task| &task.scope.files)
            .filter(|path| !path.contains(['*', '?', '[', '{']))
            .cloned()
            .collect();
        files.dedup();
        files
    })
}
