//! The guard. After each call a clear finding reminds the agent at once; a
//! suspect run is shown to the decision model first (every few steps at
//! most), and only its confident "no progress" reminds. The reminder fits
//! the failure: retry, fix the code, or fix the environment (markers first,
//! then the model; a failure after an edit defaults to the code). After the
//! configured reminders in a turn, the agent's next allowed call asks the
//! user instead (a notice only in bypass mode). It never switches models
//! or cancels the turn.

use serde_json::json;
use z_engine_context::render_template;
use z_engine_decisions::{AbstainReason, Answer, DecisionRequest, Question};
use z_engine_prompts::decisions::{LOOP_GUARD_FAILURE, LOOP_GUARD_PROGRESS};
use z_engine_prompts::reminders::{
    LOOP_FAILURE_CODE, LOOP_FAILURE_ENVIRONMENT, LOOP_FAILURE_RETRY, LOOP_NO_PROGRESS, LOOP_REPEAT,
};
use z_engine_protocol::{AgentId, NoticeLevel, PermissionMode, ToolResultPart, ToolStatus};

use super::detect::{Finding, detect};
use super::steps::{FailureClass, Step, step};
use crate::batch::ToolCall;
use crate::decisions::context::UseContext;

pub(super) const PROGRESS: &str = "loop_guard_progress";
pub(super) const FAILURE: &str = "loop_guard_failure";
pub(super) const REPEAT: &str = "loop_guard_repeat";
pub(super) const AFTER_EDIT: &str = "loop_guard_failed_after_edit";
/// Steps the decision model reads.
const DIGEST_STEPS: usize = 8;
/// The fewest new steps between two looks by the decision model.
const CHECK_EVERY: usize = 4;

/// What the decision model sees: the latest steps and the latest failure.
struct Look {
    steps: Vec<String>,
    tool: String,
    target: String,
    error: String,
}

pub(super) async fn after_call(
    cx: &UseContext,
    call: &ToolCall,
    status: ToolStatus,
    output: &[ToolResultPart],
) -> Vec<String> {
    let (Some(agent), Some(step)) = (&cx.agent, step(call, status, output)) else {
        return Vec::new();
    };
    let turn = cx.core.with_state(|state| state.turns.len());
    let watch = cx.core.decisions.loops();
    let found = watch.with(agent, turn, |history| {
        history.push(step);
        let finding = detect(&history.steps)?;
        if !finding.is_clear() {
            if !history.due(CHECK_EVERY) {
                return None;
            }
            history.checked = Some(history.seen);
        }
        Some((finding, look(&history.steps)))
    });
    let Some((finding, look)) = found else {
        return Vec::new();
    };
    let Some(confirmed) = confirm(cx, &finding, &look).await else {
        return Vec::new();
    };
    respond(cx, agent, turn, &finding, confirmed)
}

/// The reason to ask before `agent`'s next allowed call, once armed.
pub(super) fn gate(cx: &UseContext, call: &ToolCall) -> Option<String> {
    let agent = cx.agent.as_ref()?;
    let turn = cx.core.with_state(|state| state.turns.len());
    let armed =
        (cx.core.decisions.loops()).with(agent, turn, |history| std::mem::take(&mut history.ask));
    armed.then(|| {
        format!(
            "Loop guard: the agent kept repeating itself after its reminders. Allow lets this \
             {} call run; deny it with a note to tell the agent what to do instead.",
            call.name
        )
    })
}

/// A finding that stands, with the failure's class and the model's answers
/// (traced once the guard knows what it did).
#[derive(Default)]
struct Confirmed {
    class: Option<FailureClass>,
    answers: Vec<Answer>,
    fingerprint: String,
}

/// Clear findings always stand; a suspect one only on the model's confident
/// "no progress". The model sizes up a repeating failure the markers could
/// not class.
async fn confirm(cx: &UseContext, finding: &Finding, look: &Look) -> Option<Confirmed> {
    let (suspect, known, classify) = match finding {
        Finding::Repeat { .. } => return Some(Confirmed::default()),
        Finding::FailedAfterEdit { class, .. } => (false, *class, class.is_none()),
        Finding::Suspect {
            class,
            same_failure,
            ..
        } => {
            let class = class.filter(|_| *same_failure);
            (true, class, *same_failure && class.is_none())
        }
    };
    if !suspect && !classify {
        return Some(Confirmed {
            class: known,
            ..Confirmed::default()
        });
    }
    let state = json!({
        "steps": look.steps,
        "tool": look.tool,
        "target": look.target,
        "error": look.error,
    });
    let mut request = DecisionRequest::new(state);
    if suspect {
        request = request.ask(Question::yes_no(PROGRESS, LOOP_GUARD_PROGRESS).ok()?);
    }
    if classify {
        request = request.ask(Question::choice(FAILURE, LOOP_GUARD_FAILURE).ok()?);
    }
    let answers = cx.ask(&request).await;
    let fingerprint = request.fingerprint();
    let find = |name: &str| answers.iter().find(|answer| answer.question == name);
    let stuck = !suspect || find(PROGRESS).and_then(Answer::yes) == Some(false);
    if !stuck {
        for answer in &answers {
            cx.record(cx.record_of(answer, &fingerprint));
        }
        return None;
    }
    let class = known.or_else(|| find(FAILURE)?.choice().and_then(FailureClass::parse));
    Some(Confirmed {
        class,
        answers,
        fingerprint,
    })
}

/// A reminder while the turn has some left; then arms the ask.
fn respond(
    cx: &UseContext,
    agent: &AgentId,
    turn: usize,
    finding: &Finding,
    confirmed: Confirmed,
) -> Vec<String> {
    let max = cx.core.settings().settings.decisions.loop_guard.reminders();
    let bypass = cx.core.mode() == PermissionMode::Bypass;
    let given = cx.core.decisions.loops().with(agent, turn, |history| {
        if history.reminders < max {
            history.reminders += 1;
            return Some(history.reminders);
        }
        history.reminders = 0;
        history.ask = !cx.shadow && !bypass;
        None
    });
    let summary = summary(finding);
    let (outcome, notice) = match (given, bypass) {
        (Some(n), _) => (
            format!("reminded ({n} of {max})"),
            format!("Loop guard: {summary}. Reminded the agent ({n} of {max})."),
        ),
        (None, false) => (
            "asked the user".to_string(),
            format!(
                "Loop guard: {summary}, still after {max} reminders. Its next call asks you first."
            ),
        ),
        (None, true) => (
            "noticed".to_string(),
            format!(
                "Loop guard: {summary}, still after {max} reminders. Bypass mode does not ask; stop the turn if it keeps going."
            ),
        ),
    };
    record(cx, finding, &confirmed, &outcome);
    if !cx.shadow {
        cx.core.events.notice(NoticeLevel::Warn, notice);
    }
    given
        .map(|_| vec![reminder(finding, confirmed.class)])
        .unwrap_or_default()
}

/// The rule's record for a clear finding, and each model answer, all with
/// what the guard did. Fingerprints only, never the output.
fn record(cx: &UseContext, finding: &Finding, confirmed: &Confirmed, outcome: &str) {
    let rule = match finding {
        Finding::Repeat { call, .. } => Some((REPEAT, *call)),
        Finding::FailedAfterEdit { failure, .. } => Some((AFTER_EDIT, *failure)),
        Finding::Suspect { .. } => None,
    };
    if let Some((question, hash)) = rule {
        let answer = Answer::abstained(question, AbstainReason::Rules, "rules");
        let record = cx.record_of(&answer, &format!("{hash:016x}"));
        cx.record(record.outcome(outcome));
    }
    for answer in &confirmed.answers {
        let record = cx.record_of(answer, &confirmed.fingerprint);
        cx.record(record.outcome(outcome));
    }
}

fn summary(finding: &Finding) -> String {
    match finding {
        Finding::Repeat { tool, count, .. } => {
            format!("{tool} ran {count} times with the same input and result")
        }
        Finding::FailedAfterEdit { tool, count, .. } => {
            format!("{tool} failed the same way after an edit ({count} times)")
        }
        Finding::Suspect { count, .. } => format!("the last {count} steps made no progress"),
    }
}

/// Environment and retry failures get their own text; otherwise a failure
/// after an edit points at the code, and a suspect run at the approach.
pub(super) fn reminder(finding: &Finding, class: Option<FailureClass>) -> String {
    let (tool, count, after_edit) = match finding {
        Finding::Repeat { tool, count, .. } => {
            return fill(LOOP_REPEAT, tool, *count);
        }
        Finding::FailedAfterEdit { tool, count, .. } => (tool, *count, true),
        Finding::Suspect { tool, count, .. } => (tool, *count, false),
    };
    let template = match class {
        Some(FailureClass::Environment) => LOOP_FAILURE_ENVIRONMENT,
        Some(FailureClass::Retry) => LOOP_FAILURE_RETRY,
        _ if after_edit => LOOP_FAILURE_CODE,
        _ => LOOP_NO_PROGRESS,
    };
    fill(template, tool, count)
}

fn fill(template: &str, tool: &str, count: usize) -> String {
    let count = count.to_string();
    render_template(template, &[("tool", tool), ("count", count.as_str())])
}

fn look(steps: &[Step]) -> Look {
    let latest = &steps[steps.len().saturating_sub(DIGEST_STEPS)..];
    let last = steps.last();
    Look {
        steps: latest.iter().map(|step| step.label.clone()).collect(),
        tool: last.map(|step| step.tool.clone()).unwrap_or_default(),
        target: last.map(|step| step.target.clone()).unwrap_or_default(),
        error: (last.and_then(|step| step.failure.as_ref()))
            .map(|failure| failure.excerpt.clone())
            .unwrap_or_default(),
    }
}
