//! The full `SessionSnapshot` the GUI renders on open, after rewinds and
//! whenever a live session is opened again.

use z_engine_protocol::{AgentId, Event, SessionInfo, SessionSnapshot};

use crate::session::SessionCore;
use crate::work::attached;

pub(crate) fn snapshot(core: &SessionCore) -> SessionSnapshot {
    let (pending_approvals, pending_questions, pending_plans) = core.broker.snapshot();
    let jobs = core.jobs.list();
    let status = core.status.current();
    let work = attached(core).map(|plan| plan.read(|state| state.snapshot()));
    core.with_state(|state| SessionSnapshot {
        info: SessionInfo {
            session_id: core.id.clone(),
            title: state.title.clone(),
            project_root: core.root.to_string_lossy().into_owned(),
            model: state.model.clone(),
            mode: state.mode,
            planning: state.planning,
            effort: state.effort,
            created_at: state.created_at,
            updated_at: state.updated_at,
            legacy: state.legacy,
        },
        status,
        messages: state.transcript.clone(),
        compactions: state.compactions.clone(),
        turns: state.turns.clone(),
        todos: state.todos_of(&AgentId::main()).to_vec(),
        agents: state.agents.clone(),
        jobs,
        checks: state.checks.clone(),
        checkpoints: state
            .checkpoints
            .iter()
            .map(|(info, _)| info.clone())
            .collect(),
        pending_approvals,
        pending_questions,
        pending_plans,
        queued: state.queue.clone(),
        usage: state.usage,
        cost_usd: state.cost_usd,
        context_tokens: state.context_tokens,
        context_limit: state.context_limit,
        task_views: state.task_views.clone(),
        work,
    })
}

pub(crate) fn emit_snapshot(core: &SessionCore) {
    core.events.emit(Event::Snapshot {
        snapshot: Box::new(snapshot(core)),
    });
}
