//! The hybrid provider: callers apply their deterministic filters first and
//! ask only about ambiguous items; this asks the model (through the cache),
//! calibrates, and turns every failure or low-confidence answer into an
//! abstain, counting it as a fallback. It never returns an error.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::answer::{AbstainReason, Answer};
use crate::cache::{DecisionCache, cache_key};
use crate::calibration::Calibration;
use crate::error::DecisionError;
use crate::provider::DecisionProvider;
use crate::question::DecisionRequest;

#[derive(Debug)]
pub struct HybridProvider {
    model: Arc<dyn DecisionProvider>,
    calibration: Calibration,
    cache: DecisionCache,
    fallbacks: AtomicU64,
}

impl HybridProvider {
    pub fn new(model: Arc<dyn DecisionProvider>, calibration: Calibration) -> Self {
        Self {
            model,
            calibration,
            cache: DecisionCache::default(),
            fallbacks: AtomicU64::new(0),
        }
    }

    /// Answers abstained for a model failure or low confidence so far.
    pub fn fallbacks(&self) -> u64 {
        self.fallbacks.load(Ordering::Relaxed)
    }

    pub fn model(&self) -> &Arc<dyn DecisionProvider> {
        &self.model
    }

    async fn ask_model(
        &self,
        request: &DecisionRequest,
        keys: &[u64],
        cancel: &CancellationToken,
    ) -> Vec<Answer> {
        let mut answers: Vec<Option<Answer>> = keys
            .iter()
            .map(|key| {
                let mut hit = self.cache.get(*key)?;
                (hit.cached, hit.latency_ms) = (true, 0);
                Some(hit)
            })
            .collect();
        let missing: Vec<usize> = (0..keys.len()).filter(|i| answers[*i].is_none()).collect();
        if missing.is_empty() {
            return answers.into_iter().flatten().collect();
        }
        let mut uncached = DecisionRequest::new(request.state.clone());
        uncached.questions = missing
            .iter()
            .map(|i| request.questions[*i].clone())
            .collect();
        let started = Instant::now();
        let reply = self.model.decide(&uncached, cancel).await;
        let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let fresh = fresh_answers(&uncached, reply, self.model.name());
        for (index, mut answer) in missing.into_iter().zip(fresh) {
            answer.latency_ms = latency_ms;
            if answer.abstain.is_none() {
                self.cache.put(keys[index], answer.clone());
            }
            answers[index] = Some(answer);
        }
        answers.into_iter().flatten().collect()
    }

    /// Calibrates and gates one answer on its question's threshold.
    fn gate(&self, mut answer: Answer) -> Answer {
        if answer.abstain.is_none() {
            self.calibration.apply(&mut answer);
            let threshold = self.calibration.threshold_for(&answer.question);
            if answer
                .confidence
                .is_none_or(|confidence| confidence < threshold)
            {
                answer.abstain = Some(AbstainReason::LowConfidence);
            }
        }
        if answer.abstain.is_some_and(AbstainReason::is_fallback) {
            self.fallbacks.fetch_add(1, Ordering::Relaxed);
        }
        answer
    }
}

/// One answer per question: errors abstain with their reason, and a reply
/// with the wrong number of answers abstains as malformed.
fn fresh_answers(
    request: &DecisionRequest,
    reply: Result<Vec<Answer>, DecisionError>,
    provider: &str,
) -> Vec<Answer> {
    let abstain_all = |reason| {
        let all = request.questions.iter();
        all.map(|question| Answer::abstained(&question.name, reason, provider))
            .collect()
    };
    match reply {
        Ok(answers) if answers.len() == request.questions.len() => answers,
        Ok(_) => abstain_all(AbstainReason::Malformed),
        Err(error) => {
            tracing::debug!(%error, "decision model unavailable; keeping rules");
            abstain_all(AbstainReason::from_error(&error))
        }
    }
}

#[async_trait]
impl DecisionProvider for HybridProvider {
    fn name(&self) -> &'static str {
        "hybrid"
    }

    fn revision(&self) -> String {
        self.model.revision()
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        if let Err(error) = request.validate() {
            tracing::debug!(%error, "invalid decision request");
            return Ok(fresh_answers(request, Err(error), self.model.name()));
        }
        let state = request.state.to_string();
        let revision = self.model.revision();
        let keys: Vec<u64> = request
            .questions
            .iter()
            .map(|question| cache_key(question, &state, &revision))
            .collect();
        let answers = self.ask_model(request, &keys, cancel).await;
        Ok(answers
            .into_iter()
            .map(|answer| self.gate(answer))
            .collect())
    }
}
