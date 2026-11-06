//! The use. One request per task start: how complex the task is, plus "is
//! this a new task?" when the message may only continue the current one.
//! Simple means low effort and, when offered, the fast model; moderate and
//! complex mean medium and high effort. A request that looks large is never
//! routed down (traced as an override). Every answer is traced with the
//! route it chose, or would have chosen in shadow mode.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_config::FeatureId;
use z_engine_decisions::{Answer, DecisionRequest, Question, UNCHANGED};
use z_engine_prompts::decisions::{ROUTING_COMPLEXITY, ROUTING_NEW_TASK};

use super::signals::{Tier, looks_large};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::decisions::seams::{RouteAdvice, RouteTask};
use crate::decisions::uses::digest::head;

pub(super) const COMPLEXITY: &str = "routing_complexity";
pub(super) const NEW_TASK: &str = "routing_new_task";
const PROMPT_CHARS: usize = 1_200;
const PREVIOUS_CHARS: usize = 300;
const KEPT: &str =
    "kept off low effort and the fast model: the request is long or names many files";

#[derive(Debug)]
pub(crate) struct Routing;

pub(crate) static ROUTING: Routing = Routing;

#[async_trait]
impl DecisionUse for Routing {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsRouting
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::Route]
    }

    async fn route(&self, cx: &UseContext, task: &RouteTask) -> Option<RouteAdvice> {
        decide(cx, task).await
    }
}

async fn decide(cx: &UseContext, task: &RouteTask) -> Option<RouteAdvice> {
    let mut request = DecisionRequest::new(state(task))
        .ask(Question::choice(COMPLEXITY, ROUTING_COMPLEXITY).ok()?);
    if task.previous.is_some() {
        request = request.ask(Question::yes_no(NEW_TASK, ROUTING_NEW_TASK).ok()?);
    }
    let answers = cx.ask(&request).await;
    let fingerprint = request.fingerprint();
    let find = |name: &str| answers.iter().find(|answer| answer.question == name);
    let starts = match find(NEW_TASK) {
        Some(answer) => {
            let starts = answer.yes() == Some(true);
            let outcome = if starts { "new task" } else { "same task" };
            cx.record(cx.record_of(answer, &fingerprint).outcome(outcome));
            starts
        }
        None => task.previous.is_none(),
    };
    let complexity = find(COMPLEXITY)?;
    let tier = complexity.choice().and_then(Tier::parse);
    let plan = tier
        .filter(|_| starts)
        .map(|tier| plan(task, tier, complexity));
    let mut record = cx.record_of(complexity, &fingerprint);
    match &plan {
        Some((advice, kept)) => {
            record = record.outcome(&route_label(advice));
            if *kept {
                record = record.overridden(KEPT);
            }
        }
        None => record = record.outcome(UNCHANGED),
    }
    cx.record(record);
    match plan {
        Some((advice, _)) => Some(advice),
        // A confirmed new task the model could not size: back to defaults.
        None => starts.then(RouteAdvice::default),
    }
}

/// The route for `tier`, and whether the size rule kept it from going down.
fn plan(task: &RouteTask, tier: Tier, answer: &Answer) -> (RouteAdvice, bool) {
    let kept = tier == Tier::Simple && looks_large(&task.prompt);
    let used = if kept { Tier::Moderate } else { tier };
    let model = match (&task.models, used) {
        (Some((_, fast)), Tier::Simple) => Some(fast.clone()),
        _ => None,
    };
    let confidence = answer
        .confidence
        .map(|confidence| format!(", confidence {confidence:.2}"))
        .unwrap_or_default();
    let reason = if kept {
        format!("{} task{confidence}, but a large request", tier.label())
    } else {
        format!("{} task{confidence}", tier.label())
    };
    let advice = RouteAdvice {
        effort: task.effort_free.then(|| used.effort()),
        model,
        reason,
    };
    (advice, kept)
}

pub(super) fn route_label(advice: &RouteAdvice) -> String {
    let mut parts = Vec::new();
    if let Some(effort) = advice.effort {
        parts.push(format!("effort {}", effort.label()));
    }
    if advice.model.is_some() {
        parts.push("fast model".to_string());
    }
    if parts.is_empty() {
        UNCHANGED.to_string()
    } else {
        parts.join(", ")
    }
}

fn state(task: &RouteTask) -> Value {
    let mut state = json!({ "request": head(&task.prompt, PROMPT_CHARS) });
    if let Some(agent_type) = &task.agent_type {
        state["agent"] = json!(agent_type);
    }
    if let Some(previous) = &task.previous {
        state["current_task"] = json!(head(previous, PREVIOUS_CHARS));
    }
    state
}
