//! Preparing a child run: the definition (or the finished agent being
//! resumed with its transcript), where it works (shared tree or a fresh or
//! re-entered worktree, falling back to shared outside git), its
//! `AgentInfo`, its transcript sink with the task as the first new user
//! message, and its cancellation (under the caller for foreground agents,
//! under the session for background ones).

use std::sync::Arc;

use hyperform_protocol::{
    AgentId, AgentInfo, AgentStatus, CallId, Isolation, Message, NoticeLevel, Usage, WorktreeInfo,
    now_ms,
};
use hyperform_tools::SpawnRequest;

use super::blueprint::{Placement, child_spec, inherits, isolation};
use super::sink::AgentSink;
use super::tracker::AgentTracker;
use super::worktree;
use crate::decisions::route_subagent;
use crate::run::{ChildTally, ModelChoice, RunContext, TranscriptSink, WebTaint};
use crate::session::{AgentResources, SessionCore};
use crate::work;

/// A prepared, announced child that has not run yet.
pub(crate) struct Child {
    pub ctx: RunContext,
    pub sink: Arc<AgentSink>,
    pub tracker: Arc<AgentTracker>,
    pub description: String,
    pub worktree: Option<WorktreeInfo>,
    pub background: bool,
    /// Where a foreground child's usage and changes are rolled up.
    pub parent_tally: Arc<ChildTally>,
    /// Top-level and background agents take a concurrency slot; nested
    /// foreground agents run inside their caller's slot, so a full session
    /// cannot deadlock on its own children.
    pub needs_slot: bool,
    /// Totals of earlier runs of a resumed agent.
    pub base: (Usage, f64, u32),
}

/// What the child continues from, when resuming.
struct Resumed {
    prior: AgentInfo,
    history: Vec<Message>,
}

pub(crate) async fn prepare(
    parent: &RunContext,
    call_id: &CallId,
    req: &SpawnRequest,
) -> Result<Child, String> {
    let core = &parent.core;
    let registry = core.agents.registry();
    let resumed = match &req.resume {
        Some(id) => Some(resumable(core, id)?),
        None => None,
    };
    let agent_type = resumed
        .as_ref()
        .map_or(req.agent_type.as_str(), |resumed| &resumed.prior.agent_type);
    let def = registry.resolve(agent_type)?;
    let agent_id = resumed
        .as_ref()
        .map_or_else(AgentId::new, |resumed| resumed.prior.agent_id.clone());
    let wanted = match &resumed {
        Some(resumed) => resumed.prior.isolation,
        None => isolation(def, req.isolation),
    };
    let history = resumed
        .as_ref()
        .map(|resumed| resumed.history.clone())
        .unwrap_or_default();
    let sink = AgentSink::open(core, &agent_id, history).map_err(|error| error.to_string())?;
    let prior_worktree = resumed
        .as_ref()
        .and_then(|resumed| resumed.prior.worktree.as_ref());
    let (placement, worktree) = place(core, &agent_id, wanted, prior_worktree).await;
    let routed = match &resumed {
        None if inherits(def) => route_subagent(parent, &def.name, &agent_id, &req.prompt).await,
        _ => None,
    };
    let spec = child_spec(def, parent, agent_id.clone(), placement, routed.as_deref());
    let description = resumed.as_ref().map_or_else(
        || req.description.clone(),
        |resumed| resumed.prior.description.clone(),
    );
    let base = resumed
        .as_ref()
        .map_or((Usage::default(), 0.0, 0), |resumed| {
            let prior = &resumed.prior;
            (prior.usage, prior.cost_usd, prior.tool_calls)
        });
    let info = AgentInfo {
        agent_id: agent_id.clone(),
        parent_id: Some(parent.spec.agent_id.clone()),
        call_id: Some(call_id.clone()),
        agent_type: def.name.clone(),
        description: description.clone(),
        model: match &spec.model {
            ModelChoice::Fixed(model) => model.clone(),
            ModelChoice::Session => core.main_model(),
        },
        background: req.background,
        isolation: if worktree.is_some() {
            Isolation::Worktree
        } else {
            Isolation::Shared
        },
        worktree: worktree.clone(),
        status: AgentStatus::Running,
        depth: spec.depth,
        started_at: now_ms(),
        finished_at: None,
        usage: base.0,
        cost_usd: base.1,
        tool_calls: base.2,
        result_preview: None,
        error: None,
        read_web: resumed
            .as_ref()
            .is_some_and(|resumed| resumed.prior.read_web),
        task_id: req.task_id.as_deref().map(work::task_id),
    };
    let sink = Arc::new(sink);
    sink.append(&Message::user_text(req.prompt.clone()), false)
        .map_err(|error| error.to_string())?;
    let web = WebTaint::inherit(&parent.web);
    if info.read_web {
        web.mark();
    }
    let tracker = Arc::new(AgentTracker::new(info, Arc::clone(&web)));
    tracker.announce(core);
    let cancel = if req.background {
        core.cancel.child_token()
    } else {
        parent.cancel.child_token()
    };
    let resources = AgentResources::new(spec.root.clone());
    let mut ctx = RunContext::new(Arc::clone(core), spec, resources, cancel);
    ctx.tracker = Some(Arc::clone(&tracker));
    ctx.web = web;
    Ok(Child {
        ctx,
        sink,
        tracker,
        description,
        worktree,
        background: req.background,
        parent_tally: Arc::clone(&parent.children),
        needs_slot: req.background || parent.spec.is_main(),
        base,
    })
}

fn resumable(core: &SessionCore, id: &AgentId) -> Result<Resumed, String> {
    let prior = core
        .with_state(|state| {
            let found = state.agents.iter().find(|info| info.agent_id == *id);
            found.cloned()
        })
        .ok_or_else(|| format!("no agent {id} to resume in this session"))?;
    if !prior.status.is_terminal() {
        return Err(format!(
            "agent {id} is still running; wait for it to finish"
        ));
    }
    let history = core
        .shared
        .store
        .load_agent_transcript(&core.id, id)
        .map_err(|error| format!("could not load agent {id}'s transcript: {error}"))?;
    Ok(Resumed { prior, history })
}

/// The shared tree, or a worktree (re-entered when a resumed agent's is
/// still pending). Outside git, or when git fails, the agent shares the
/// tree and the user is told.
async fn place(
    core: &SessionCore,
    agent: &AgentId,
    wanted: Isolation,
    prior: Option<&WorktreeInfo>,
) -> (Placement, Option<WorktreeInfo>) {
    let shared = Placement {
        root: core.root.clone(),
        worktree: None,
    };
    if wanted == Isolation::Shared {
        return (shared, None);
    }
    if let Some(prior) = prior {
        if let Some(placement) = worktree::reenter(core, prior).await {
            return (placement, Some(prior.clone()));
        }
    }
    let reason = match worktree::prepare(core, agent).await {
        Ok(Some((placement, info))) => return (placement, Some(info)),
        Ok(None) => "the project is not a git repository".to_string(),
        Err(error) => format!("the worktree could not be created: {error}"),
    };
    core.events.notice(
        NoticeLevel::Info,
        format!("Agent {agent} runs in the shared project tree because {reason}."),
    );
    (shared, None)
}
