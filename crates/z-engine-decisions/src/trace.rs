//! A per-session ring buffer of decision records for the Context tab.
//! Records hold fingerprints, labels and numbers, never source text.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use z_engine_protocol::now_ms;

use crate::answer::Answer;

const DEFAULT_CAPACITY: usize = 500;
/// Outcome of a record whose use left today's behavior in place.
pub const UNCHANGED: &str = "unchanged";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRecord {
    /// Assigned by the trace, increasing within a session.
    pub seq: u64,
    pub at_ms: u64,
    /// The feature id that asked (`decisions_compaction`, ...).
    pub feature: String,
    pub question: String,
    /// Recorded in shadow mode: what would have happened.
    pub shadow: bool,
    pub provider: String,
    /// Digest of the input state, never the input itself.
    pub input_fingerprint: String,
    /// The model's proposal label, even when it was not acted on.
    pub answer: Option<String>,
    pub confidence: Option<f64>,
    pub latency_ms: u64,
    pub cached: bool,
    /// Why the answer was not usable (`timeout`, `low confidence`, ...).
    pub fallback: Option<String>,
    /// What the kernel finally did (`kept`, `cleared`, `asked`, ...).
    pub outcome: String,
    /// Why the kernel overrode the proposal ("file named by the user").
    pub override_reason: Option<String>,
    /// Tokens saved, or that would have been saved in shadow mode.
    pub tokens_saved: Option<u64>,
}

impl DecisionRecord {
    /// A record of `answer`; the outcome starts as [`UNCHANGED`].
    pub fn of(feature: &str, answer: &Answer, fingerprint: &str, shadow: bool) -> Self {
        Self {
            seq: 0,
            at_ms: 0,
            feature: feature.to_string(),
            question: answer.question.clone(),
            shadow,
            provider: answer.provider.clone(),
            input_fingerprint: fingerprint.to_string(),
            answer: answer.proposal.as_ref().map(|verdict| verdict.label()),
            confidence: answer.confidence,
            latency_ms: answer.latency_ms,
            cached: answer.cached,
            fallback: answer.abstain.map(|reason| reason.label().to_string()),
            outcome: UNCHANGED.to_string(),
            override_reason: None,
            tokens_saved: None,
        }
    }

    pub fn outcome(mut self, outcome: &str) -> Self {
        self.outcome = outcome.to_string();
        self
    }

    pub fn overridden(mut self, reason: &str) -> Self {
        self.override_reason = Some(reason.to_string());
        self
    }

    pub fn saved(mut self, tokens: u64) -> Self {
        self.tokens_saved = Some(tokens);
        self
    }
}

/// Totals over the whole session; latency percentiles over recent records.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionSummary {
    pub count: u64,
    pub shadow: u64,
    pub fallbacks: u64,
    pub p50_ms: Option<u64>,
    pub p95_ms: Option<u64>,
    pub tokens_saved: u64,
    pub would_save_tokens: u64,
}

#[derive(Debug)]
pub struct DecisionTrace {
    capacity: usize,
    inner: Mutex<Ring>,
}

#[derive(Debug, Default)]
struct Ring {
    records: VecDeque<DecisionRecord>,
    next_seq: u64,
    totals: DecisionSummary,
}

impl Default for DecisionTrace {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl DecisionTrace {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            inner: Mutex::default(),
        }
    }

    /// Stamps the sequence number and time, then appends, dropping the
    /// oldest record when full.
    pub fn record(&self, mut record: DecisionRecord) {
        let mut ring = self.lock();
        ring.next_seq += 1;
        record.seq = ring.next_seq;
        record.at_ms = now_ms();
        let totals = &mut ring.totals;
        totals.count += 1;
        totals.shadow += u64::from(record.shadow);
        totals.fallbacks += u64::from(record.fallback.as_deref().is_some_and(|f| f != "rules"));
        let saved = record.tokens_saved.unwrap_or_default();
        match record.shadow {
            true => totals.would_save_tokens += saved,
            false => totals.tokens_saved += saved,
        }
        ring.records.push_back(record);
        if ring.records.len() > self.capacity {
            ring.records.pop_front();
        }
    }

    /// The newest `limit` records, newest first.
    pub fn recent(&self, limit: usize) -> Vec<DecisionRecord> {
        self.lock()
            .records
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn summary(&self) -> DecisionSummary {
        let ring = self.lock();
        let mut latencies: Vec<u64> = ring
            .records
            .iter()
            .filter(|record| !record.cached && record.fallback.as_deref() != Some("rules"))
            .map(|record| record.latency_ms)
            .collect();
        latencies.sort_unstable();
        DecisionSummary {
            p50_ms: percentile(&latencies, 50),
            p95_ms: percentile(&latencies, 95),
            ..ring.totals.clone()
        }
    }

    /// Records are appended whole, so a poisoned ring is still consistent.
    fn lock(&self) -> std::sync::MutexGuard<'_, Ring> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Nearest-rank percentile of sorted values.
fn percentile(sorted: &[u64], percent: usize) -> Option<u64> {
    let rank = (sorted.len() * percent).div_ceil(100).max(1);
    sorted.get(rank - 1).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::{AbstainReason, Verdict};

    fn answer(latency_ms: u64, abstain: Option<AbstainReason>) -> Answer {
        let mut answer = Answer::abstained("relevant", AbstainReason::Rules, "systemone");
        answer.abstain = abstain;
        answer.proposal = Some(Verdict::YesNo(false));
        answer.confidence = Some(0.94);
        answer.latency_ms = latency_ms;
        answer
    }

    #[test]
    fn records_sum_savings_and_fallbacks_and_keep_the_newest() {
        let trace = DecisionTrace::new(3);
        for latency in [10, 20, 30, 40] {
            let record =
                DecisionRecord::of("decisions_compaction", &answer(latency, None), "ab", false);
            trace.record(record.outcome("cleared").saved(100));
        }
        let shadow = DecisionRecord::of("decisions_compaction", &answer(5, None), "cd", true);
        trace.record(shadow.saved(50));
        let timeout = answer(0, Some(AbstainReason::Timeout));
        trace.record(DecisionRecord::of("decisions_risk", &timeout, "ef", false));
        let recent = trace.recent(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].seq, 6);
        assert_eq!(recent[0].fallback.as_deref(), Some("timeout"));
        assert_eq!(recent[1].answer.as_deref(), Some("no"));
        let summary = trace.summary();
        assert_eq!(
            (summary.count, summary.shadow, summary.fallbacks),
            (6, 1, 1)
        );
        assert_eq!((summary.tokens_saved, summary.would_save_tokens), (400, 50));
        assert_eq!((summary.p50_ms, summary.p95_ms), (Some(5), Some(40)));
    }

    #[test]
    fn records_serialize_camel_case_without_inputs() {
        let record = DecisionRecord::of("decisions_risk", &answer(12, None), "ab", true)
            .overridden("file named by the user");
        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["inputFingerprint"], "ab");
        assert_eq!(json["overrideReason"], "file named by the user");
        assert_eq!(json["latencyMs"], 12);
        assert!(json.get("state").is_none());
    }

    #[test]
    fn percentiles_use_the_nearest_rank() {
        assert_eq!(percentile(&[], 50), None);
        assert_eq!(percentile(&[7], 95), Some(7));
        assert_eq!(percentile(&[1, 2, 3, 4], 50), Some(2));
    }
}
