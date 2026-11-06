//! What agents produce while working on a task, recorded without a work
//! tool call: check runs become evidence on the check criteria of the
//! task the running agent owns (the engine links them, so a passing check
//! marks a criterion Met and a failing one Failed), and `TodoWrite` lists
//! become the steps of that task. The main agent's checks run in the
//! project tree, so they also count for tasks Verifying there, which then
//! complete or fail as after a merge.

use z_engine_protocol::work::{Actor, CriterionMethod, Task, TaskStatus, WorkCause};
use z_engine_protocol::{AgentId, CheckRecord, TaskId, TodoItem};
use z_engine_tools::names;
use z_engine_work::{Proposal, Submission};

use super::coordinate::{News, tell_coordinator};
use super::facts::{agent, instant, owned_active};
use super::helper::check_evidence;
use super::lifecycle::{attached, report};
use super::merged::complete;
use super::plan::Submitted;
use crate::session::SessionCore;

pub(crate) async fn link_check(core: &SessionCore, record: &CheckRecord) {
    let Some(plan) = attached(core) else {
        return;
    };
    let actor = agent(core, &record.agent_id);
    let mut facts = instant(core);
    facts.fingerprint = Some(record.fingerprint_after.clone());
    let verifying: Vec<(TaskId, AgentId)> = if record.agent_id.is_main() {
        plan.read(|state| {
            let verifying = state.tasks.values().filter(|task| {
                task.status == TaskStatus::Verifying && !check_links(core, task, record).is_empty()
            });
            verifying
                .map(|task| (task.id.clone(), helper_of(task)))
                .collect()
        })
    } else {
        Vec::new()
    };
    let result = plan.submit(&facts, |state| {
        let own = owned_active(state, &actor).into_iter();
        let others = verifying.iter().filter_map(|(id, _)| state.task(id));
        let proposals = own
            .chain(others)
            .flat_map(|task| check_links(core, task, record))
            .collect();
        Ok(Submission::new(Actor::Kernel, cause(), proposals))
    });
    let linked = matches!(result, Ok(Submitted::Accepted(_)));
    report(core, result, "The check was not linked to the task");
    if !linked {
        return;
    }
    for (task, helper) in verifying {
        let news = complete(core, &plan, &task, std::slice::from_ref(record)).await;
        if matches!(news, News::Verified | News::CheckFailed) {
            tell_coordinator(core, &helper, &task, news).await;
        }
    }
}

/// Evidence links from `record` to each of `task`'s criteria checked by
/// its kind.
pub(crate) fn check_links(core: &SessionCore, task: &Task, record: &CheckRecord) -> Vec<Proposal> {
    task.acceptance
        .iter()
        .filter(|criterion| {
            matches!(criterion.method, CriterionMethod::Check { kind } if kind == record.kind)
        })
        .map(|criterion| Proposal::LinkEvidence {
            task_id: task.id.clone(),
            criterion_id: Some(criterion.id.clone()),
            evidence: check_evidence(core, record),
        })
        .collect()
}

/// The helper a task was dispatched to, else the main agent.
pub(crate) fn helper_of(task: &Task) -> AgentId {
    task.run
        .dispatch
        .as_ref()
        .map_or_else(AgentId::main, |dispatch| dispatch.agent_id.clone())
}

pub(crate) fn cause() -> WorkCause {
    WorkCause::Tool {
        tool: names::VERIFY.to_string(),
        call_id: None,
    }
}

/// The agent's todo list, as the steps of the task it works on;
/// `untrusted` when the agent's run read web content.
pub(crate) fn record_steps(
    core: &SessionCore,
    agent_id: &AgentId,
    todos: &[TodoItem],
    cause: WorkCause,
    untrusted: bool,
) {
    let Some(plan) = attached(core) else {
        return;
    };
    let actor = agent(core, agent_id);
    let mut facts = instant(core);
    facts.untrusted_input = untrusted;
    let result = plan.submit(&facts, |state| {
        let proposals = owned_active(state, &actor)
            .filter(|task| task.steps != todos)
            .map(|task| Proposal::RecordSteps {
                task_id: task.id.clone(),
                steps: todos.to_vec(),
            })
            .into_iter()
            .collect();
        Ok(Submission::new(actor.clone(), cause, proposals))
    });
    report(core, result, "The task steps were not recorded");
}
