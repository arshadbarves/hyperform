//! Asking a set of rules about one state in one request. Each rule becomes
//! a question named after it (`rule_<name>`, so it can be calibrated); a
//! rule fires only on a confident answer listed in its `when`.

use serde_json::Value;
use z_engine_config::DecisionRule;
use z_engine_context::render_template;
use z_engine_decisions::{Criterion, DecisionRequest, Form, Question, UNCHANGED, Verdict};

use crate::decisions::context::UseContext;

const NAME_CHARS: usize = 40;

/// A rule whose answer triggers its action.
#[derive(Debug, Clone)]
pub(super) struct Fired {
    pub rule: DecisionRule,
    pub answer: String,
}

/// The rules among `rules` whose confident answer about `state` is in
/// their `when` list; `outcome` names what firing does, for the trace.
pub(super) async fn fired(
    cx: &UseContext,
    rules: Vec<DecisionRule>,
    state: Value,
    preamble: &str,
    outcome: impl Fn(&DecisionRule) -> &'static str,
) -> Vec<Fired> {
    if rules.is_empty() {
        return Vec::new();
    }
    let names = question_names(&rules);
    let mut request = DecisionRequest::new(state);
    for (rule, name) in rules.iter().zip(&names) {
        request = request.ask(question(rule, name, preamble));
    }
    let answers = cx.ask(&request).await;
    let fingerprint = request.fingerprint();
    let mut fired = Vec::new();
    for (rule, name) in rules.into_iter().zip(&names) {
        let Some(answer) = answers.iter().find(|answer| &answer.question == name) else {
            continue;
        };
        let given = match answer.verdict() {
            Some(Verdict::YesNo(yes)) => Some(if *yes { "yes" } else { "no" }.to_string()),
            Some(Verdict::Choice(key)) => Some(key.clone()),
            _ => None,
        };
        let fires = given
            .as_ref()
            .is_some_and(|given| rule.triggers().contains(given));
        let traced = if fires { outcome(&rule) } else { UNCHANGED };
        cx.record(cx.record_of(answer, &fingerprint).outcome(traced));
        if let (true, Some(answer)) = (fires, given) {
            fired.push(Fired { rule, answer });
        }
    }
    fired
}

fn question(rule: &DecisionRule, name: &str, preamble: &str) -> Question {
    let instructions = render_template(preamble, &[("question", &rule.question)]);
    let form = if rule.options.is_empty() {
        Form::YesNo {
            yes: "Yes.".into(),
            no: "No.".into(),
        }
    } else {
        let options = rule.options.iter().map(|option| Criterion {
            key: option.clone(),
            description: option.clone(),
        });
        Form::Choice(options.collect())
    };
    Question {
        name: name.to_string(),
        instructions: instructions.trim().to_string(),
        form,
    }
}

/// `rule_<slug of the label>`, numbered when two rules share one.
pub(super) fn question_names(rules: &[DecisionRule]) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(rules.len());
    for rule in rules {
        let base = format!("rule_{}", slug(rule.label()));
        let mut name = base.clone();
        let mut n = 2;
        while names.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        names.push(name);
    }
    names
}

fn slug(text: &str) -> String {
    let mut slug = String::new();
    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
        } else if !slug.is_empty() && !slug.ends_with('_') {
            slug.push('_');
        }
        if slug.len() >= NAME_CHARS {
            break;
        }
    }
    let slug = slug.trim_end_matches('_');
    if slug.is_empty() {
        "custom".into()
    } else {
        slug.into()
    }
}
