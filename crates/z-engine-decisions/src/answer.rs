//! Answers: what the model proposed, how sure it was, and whether the
//! caller must keep today's behavior (an abstained answer).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::error::DecisionError;

/// A proposal. Choice and score answers carry option keys.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Choice(String),
    YesNo(bool),
    /// 0 (lowest level) to 1 (highest level).
    Score(f64),
}

impl Verdict {
    /// Short display text for traces: the option key, `yes`/`no`, or the score.
    pub fn label(&self) -> String {
        match self {
            Self::Choice(key) => key.clone(),
            Self::YesNo(true) => "yes".to_string(),
            Self::YesNo(false) => "no".to_string(),
            Self::Score(score) => format!("{score:.2}"),
        }
    }
}

/// Why an answer must not be acted on. Every reason means "keep today's
/// behavior"; all but `Rules` count as a fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbstainReason {
    /// No model is configured; deterministic rules decide.
    Rules,
    Timeout,
    Unavailable,
    Malformed,
    LowConfidence,
    Cancelled,
    /// The question or endpoint was rejected before asking.
    Invalid,
}

impl AbstainReason {
    pub fn from_error(error: &DecisionError) -> Self {
        match error {
            DecisionError::Timeout(_) => Self::Timeout,
            DecisionError::Unavailable(_) | DecisionError::Status { .. } => Self::Unavailable,
            DecisionError::Malformed(_) => Self::Malformed,
            DecisionError::Cancelled => Self::Cancelled,
            DecisionError::InvalidQuestion(_)
            | DecisionError::InvalidEndpoint { .. }
            | DecisionError::RemoteNotAllowed(_) => Self::Invalid,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Rules => "rules",
            Self::Timeout => "timeout",
            Self::Unavailable => "unavailable",
            Self::Malformed => "malformed",
            Self::LowConfidence => "low confidence",
            Self::Cancelled => "cancelled",
            Self::Invalid => "invalid",
        }
    }

    /// A model failure or low confidence, as opposed to rules by design.
    pub fn is_fallback(self) -> bool {
        self != Self::Rules
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// The question's name in its request.
    pub question: String,
    /// What the model said, kept for traces even when abstained.
    pub proposal: Option<Verdict>,
    /// Option key (`yes`/`no` for yes/no questions) to probability.
    pub probabilities: BTreeMap<String, f64>,
    /// After calibration; the hybrid provider gates on it.
    pub confidence: Option<f64>,
    /// Set when the caller must keep today's behavior.
    pub abstain: Option<AbstainReason>,
    pub provider: String,
    pub latency_ms: u64,
    pub cached: bool,
}

impl Answer {
    pub fn abstained(question: &str, reason: AbstainReason, provider: &str) -> Self {
        Self {
            question: question.to_string(),
            proposal: None,
            probabilities: BTreeMap::new(),
            confidence: None,
            abstain: Some(reason),
            provider: provider.to_string(),
            latency_ms: 0,
            cached: false,
        }
    }

    /// The proposal a caller may act on: `None` when abstained.
    pub fn verdict(&self) -> Option<&Verdict> {
        self.abstain
            .is_none()
            .then_some(self.proposal.as_ref())
            .flatten()
    }

    pub fn yes(&self) -> Option<bool> {
        match self.verdict()? {
            Verdict::YesNo(yes) => Some(*yes),
            _ => None,
        }
    }

    pub fn choice(&self) -> Option<&str> {
        match self.verdict()? {
            Verdict::Choice(key) => Some(key),
            _ => None,
        }
    }

    pub fn score(&self) -> Option<f64> {
        match self.verdict()? {
            Verdict::Score(score) => Some(*score),
            _ => None,
        }
    }

    pub fn is_abstain(&self) -> bool {
        self.abstain.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abstained_answers_hide_their_proposal() {
        let mut answer = Answer::abstained("q", AbstainReason::LowConfidence, "systemone");
        answer.proposal = Some(Verdict::YesNo(true));
        assert_eq!(answer.yes(), None);
        answer.abstain = None;
        assert_eq!(answer.yes(), Some(true));
        assert_eq!(answer.choice(), None);
        assert_eq!(Verdict::Score(0.5).label(), "0.50");
    }

    #[test]
    fn errors_map_to_fallback_reasons() {
        let reason = AbstainReason::from_error(&DecisionError::Timeout(250));
        assert_eq!(reason, AbstainReason::Timeout);
        assert!(reason.is_fallback() && !AbstainReason::Rules.is_fallback());
    }
}
