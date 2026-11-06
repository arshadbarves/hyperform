//! Per-question temperatures and thresholds. Laya ships over-confident;
//! temperatures fitted on shadow data (`[decisions.calibration]`) bring its
//! confidence in line with how often it is right.

use std::collections::BTreeMap;

use crate::answer::{Answer, Verdict};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestionCalibration {
    pub temperature: f64,
    /// Unset uses the default threshold.
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Calibration {
    /// Confidence an answer needs before callers may act on it.
    pub threshold: f64,
    /// By question name.
    pub questions: BTreeMap<String, QuestionCalibration>,
}

impl Calibration {
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            questions: BTreeMap::new(),
        }
    }

    pub fn with(mut self, question: &str, calibration: QuestionCalibration) -> Self {
        self.questions.insert(question.to_string(), calibration);
        self
    }

    pub fn threshold_for(&self, question: &str) -> f64 {
        self.questions
            .get(question)
            .and_then(|entry| entry.threshold)
            .unwrap_or(self.threshold)
    }

    fn temperature_for(&self, question: &str) -> f64 {
        self.questions
            .get(question)
            .map_or(1.0, |entry| entry.temperature)
    }

    /// Temperature-scales the distribution and sets the confidence to the
    /// calibrated probability of the proposal. Without a distribution the
    /// reported confidence is scaled as a two-way split.
    pub fn apply(&self, answer: &mut Answer) {
        let temperature = self.temperature_for(&answer.question);
        if (temperature - 1.0).abs() < f64::EPSILON || answer.proposal.is_none() {
            return;
        }
        if answer.probabilities.is_empty() {
            answer.confidence = answer
                .confidence
                .map(|confidence| binary(confidence, temperature));
            return;
        }
        answer.probabilities = scaled(&answer.probabilities, temperature);
        let key = match &answer.proposal {
            Some(Verdict::Choice(key)) => Some(key.as_str()),
            Some(Verdict::YesNo(true)) => Some("yes"),
            Some(Verdict::YesNo(false)) => Some("no"),
            Some(Verdict::Score(_)) | None => None,
        };
        let calibrated = key.and_then(|key| answer.probabilities.get(key).copied());
        answer.confidence = calibrated.or(answer.confidence);
    }
}

fn scaled(probabilities: &BTreeMap<String, f64>, temperature: f64) -> BTreeMap<String, f64> {
    let powered: BTreeMap<String, f64> = probabilities
        .iter()
        .map(|(key, p)| (key.clone(), p.max(0.0).powf(1.0 / temperature)))
        .collect();
    let total: f64 = powered.values().sum();
    if total <= 0.0 || !total.is_finite() {
        return probabilities.clone();
    }
    powered
        .into_iter()
        .map(|(key, p)| (key, p / total))
        .collect()
}

fn binary(confidence: f64, temperature: f64) -> f64 {
    let yes = confidence.clamp(0.0, 1.0).powf(1.0 / temperature);
    let no = (1.0 - confidence.clamp(0.0, 1.0)).powf(1.0 / temperature);
    if yes + no > 0.0 {
        yes / (yes + no)
    } else {
        confidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::AbstainReason;

    fn answer(confidence: f64, probabilities: &[(&str, f64)]) -> Answer {
        let mut answer = Answer::abstained("q", AbstainReason::Rules, "test");
        answer.abstain = None;
        answer.proposal = Some(Verdict::YesNo(true));
        answer.confidence = Some(confidence);
        answer.probabilities = probabilities
            .iter()
            .map(|(key, p)| (key.to_string(), *p))
            .collect();
        answer
    }

    #[test]
    fn temperature_above_one_softens_confidence() {
        let soft = QuestionCalibration {
            temperature: 2.0,
            threshold: Some(0.9),
        };
        let calibration = Calibration::new(0.8).with("q", soft);
        let mut with_dist = answer(0.9, &[("yes", 0.9), ("no", 0.1)]);
        calibration.apply(&mut with_dist);
        let confidence = with_dist.confidence.unwrap();
        assert!((confidence - 0.75).abs() < 1e-9, "{confidence}");
        let mut bare = answer(0.9, &[]);
        calibration.apply(&mut bare);
        assert!((bare.confidence.unwrap() - 0.75).abs() < 1e-9);
        assert_eq!(calibration.threshold_for("q"), 0.9);
        assert_eq!(calibration.threshold_for("other"), 0.8);
    }

    #[test]
    fn uncalibrated_questions_are_unchanged() {
        let mut plain = answer(0.6, &[("yes", 0.6), ("no", 0.4)]);
        Calibration::new(0.8).apply(&mut plain);
        assert_eq!(plain.confidence, Some(0.6));
    }
}
