//! Skill and agent hints (`decisions_hints`): at turn start, whether an
//! installed skill or custom agent fits the request. Two steps, because
//! the decision model is weak above about 20 options: the candidates
//! sharing the most words with the request (at most six), then one yes/no
//! per candidate. Up to two confident yeses become a reminder naming them;
//! the agent still decides. Each one is hinted at most once a session.

use async_trait::async_trait;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_context::render_template;
use z_engine_decisions::{Question, UNCHANGED};
use z_engine_prompts::decisions::HINTS_FIT;
use z_engine_prompts::reminders::SKILL_HINTS;

use super::digest::head;
use super::guidance::{ask_each, content_words, overlap, word_count};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

pub(super) const QUESTION: &str = "hints_fit";
const REQUEST_CHARS: usize = 600;
const DESCRIPTION_CHARS: usize = 300;
const MIN_WORDS: usize = 5;
const MAX_ASKED: usize = 6;
const MAX_HINTS: usize = 2;

#[derive(Debug)]
pub(crate) struct Hints;

pub(crate) static HINTS: Hints = Hints;

#[derive(Debug, Clone)]
struct Candidate {
    kind: &'static str,
    name: String,
    description: String,
}

impl Candidate {
    fn key(&self) -> String {
        format!("{} {}", self.kind, self.name)
    }
}

#[async_trait]
impl DecisionUse for Hints {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsHints
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::TurnStart]
    }

    async fn turn_start(&self, cx: &UseContext, text: &str) -> Vec<String> {
        hint(cx, text).await.into_iter().collect()
    }
}

async fn hint(cx: &UseContext, text: &str) -> Option<String> {
    if word_count(text) < MIN_WORDS {
        return None;
    }
    let candidates = shortlist(cx, text);
    if candidates.is_empty() {
        return None;
    }
    let question = Question::yes_no(QUESTION, HINTS_FIT).ok()?;
    let request = head(text.trim(), REQUEST_CHARS);
    let states = candidates.iter().map(|candidate| {
        let about = head(&candidate.description, DESCRIPTION_CHARS);
        json!({ "request": request, "candidate": format!("{}: {about}", candidate.key()) })
    });
    let answers = ask_each(cx, &question, states.collect()).await;
    let mut fits: Vec<(usize, f64)> = answers
        .iter()
        .enumerate()
        .filter(|(_, (_, answer))| answer.yes() == Some(true))
        .map(|(index, (_, answer))| (index, answer.confidence.unwrap_or_default()))
        .collect();
    fits.sort_by(|a, b| b.1.total_cmp(&a.1));
    fits.truncate(MAX_HINTS);
    let chosen: Vec<usize> = fits.into_iter().map(|(index, _)| index).collect();
    for (index, (fingerprint, answer)) in answers.iter().enumerate() {
        let outcome = if chosen.contains(&index) {
            "hinted"
        } else {
            UNCHANGED
        };
        cx.record(cx.record_of(answer, fingerprint).outcome(outcome));
    }
    let memo = cx.core.decisions.memo();
    let lines: Vec<String> = chosen
        .iter()
        .map(|&index| &candidates[index])
        .inspect(|candidate| {
            memo.insert(cx.feature, &candidate.key());
        })
        .map(|candidate| {
            let about = head(&candidate.description, DESCRIPTION_CHARS);
            format!("- {} `{}`: {about}", candidate.kind, candidate.name)
        })
        .collect();
    (!lines.is_empty()).then(|| render_template(SKILL_HINTS, &[("hints", &lines.join("\n"))]))
}

/// Installed skills and custom agents not hinted yet, ranked by the words
/// they share with the request; those sharing none are left out.
fn shortlist(cx: &UseContext, text: &str) -> Vec<Candidate> {
    let settings = cx.core.settings();
    let extensions = &settings.extensions;
    let skills = extensions.skills.iter().map(|skill| Candidate {
        kind: "skill",
        name: skill.name.clone(),
        description: skill.description.clone(),
    });
    let agents = extensions.agents.iter().map(|agent| Candidate {
        kind: "agent",
        name: agent.name.clone(),
        description: agent.description.clone(),
    });
    let words = content_words(text);
    let memo = cx.core.decisions.memo();
    let mut ranked: Vec<(usize, Candidate)> = skills
        .chain(agents)
        .filter(|candidate| !memo.contains(cx.feature, &candidate.key()))
        .map(|candidate| {
            let about = format!("{} {}", candidate.name, candidate.description);
            (overlap(&words, &content_words(&about)), candidate)
        })
        .filter(|(shared, _)| *shared > 0)
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    ranked.truncate(MAX_ASKED);
    ranked.into_iter().map(|(_, candidate)| candidate).collect()
}

#[cfg(test)]
#[path = "hints_tests.rs"]
mod tests;
