//! Benchmark metrics over one provider's answers to one kind: precision
//! and recall of the acting labels, expected calibration error, latency
//! percentiles and abstain rate; and fitting a per-question temperature
//! and threshold to paste into `[decisions.calibration.<question>]`.

use z_engine_decisions::{Answer, Verdict};

/// One fixture answered: the gold label, what came back, and how fast.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub label: String,
    /// `None` when the provider abstained or failed.
    pub predicted: Option<String>,
    pub confidence: Option<f64>,
    pub latency_ms: u64,
}

impl Scored {
    pub fn of(label: &str, answer: Option<&Answer>, latency_ms: u64) -> Self {
        let usable = answer.filter(|answer| answer.abstain.is_none());
        let predicted = usable.and_then(|answer| match answer.proposal.as_ref()? {
            Verdict::YesNo(yes) => Some(if *yes { "yes" } else { "no" }.to_string()),
            Verdict::Choice(key) => Some(key.clone()),
            Verdict::Score(_) => None,
        });
        Self {
            label: label.to_string(),
            predicted,
            confidence: usable.and_then(|answer| answer.confidence),
            latency_ms,
        }
    }

    fn correct(&self) -> bool {
        self.predicted.as_deref() == Some(self.label.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub precision: Option<f64>,
    pub recall: f64,
    pub ece: Option<f64>,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub abstain_rate: f64,
}

const ECE_BINS: usize = 10;

pub fn report(scored: &[Scored], act: &[&str]) -> Report {
    let acts = |label: Option<&str>| label.is_some_and(|label| act.contains(&label));
    let predicted_act = scored
        .iter()
        .filter(|s| acts(s.predicted.as_deref()))
        .count();
    let gold_act = scored.iter().filter(|s| acts(Some(&s.label))).count();
    let hits = scored
        .iter()
        .filter(|s| acts(s.predicted.as_deref()) && s.correct())
        .count();
    let abstained = scored.iter().filter(|s| s.predicted.is_none()).count();
    let mut latencies: Vec<u64> = scored.iter().map(|s| s.latency_ms).collect();
    latencies.sort_unstable();
    Report {
        precision: (predicted_act > 0).then(|| hits as f64 / predicted_act as f64),
        recall: if gold_act == 0 {
            1.0
        } else {
            hits as f64 / gold_act as f64
        },
        ece: ece(scored),
        p50_ms: percentile(&latencies, 50),
        p95_ms: percentile(&latencies, 95),
        abstain_rate: ratio(abstained, scored.len()),
    }
}

/// Weighted gap between confidence and accuracy over ten confidence bins,
/// on the answers that carry a confidence.
fn ece(scored: &[Scored]) -> Option<f64> {
    let rated: Vec<(f64, bool)> = scored
        .iter()
        .filter_map(|s| Some((s.confidence?, s.correct())))
        .collect();
    if rated.is_empty() {
        return None;
    }
    let mut bins = [(0.0_f64, 0_usize, 0_usize); ECE_BINS];
    for (confidence, ok) in &rated {
        let bin = ((confidence * ECE_BINS as f64).ceil() as usize).clamp(1, ECE_BINS) - 1;
        bins[bin].0 += confidence;
        bins[bin].1 += 1;
        bins[bin].2 += usize::from(*ok);
    }
    let gap = bins
        .iter()
        .filter(|(_, count, _)| *count > 0)
        .map(|(sum, count, right)| {
            let n = *count as f64;
            (sum / n - *right as f64 / n).abs() * n / rated.len() as f64
        })
        .sum();
    Some(gap)
}

fn percentile(sorted: &[u64], at: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let index = (sorted.len() * at).div_ceil(100).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

fn ratio(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

/// Two-way temperature scaling, as `Calibration::apply` does without a distribution.
pub fn scale(confidence: f64, temperature: f64) -> f64 {
    let p = confidence.clamp(1e-6, 1.0 - 1e-6).powf(1.0 / temperature);
    let q = (1.0 - confidence)
        .clamp(1e-6, 1.0 - 1e-6)
        .powf(1.0 / temperature);
    p / (p + q)
}

/// The temperature in 0.5..=4.0 (steps of 0.1) with the lowest negative
/// log-likelihood of the model's confident-or-not answers being right.
pub fn fit_temperature(scored: &[Scored]) -> f64 {
    let rated: Vec<(f64, bool)> = scored
        .iter()
        .filter_map(|s| Some((s.confidence?, s.correct())))
        .collect();
    let nll = |t: f64| -> f64 {
        rated
            .iter()
            .map(|(c, ok)| {
                let p = scale(*c, t).clamp(1e-6, 1.0 - 1e-6);
                -(if *ok { p } else { 1.0 - p }).ln()
            })
            .sum()
    };
    (5..=40)
        .map(|step| step as f64 / 10.0)
        .min_by(|a, b| nll(*a).total_cmp(&nll(*b)))
        .unwrap_or(1.0)
}

/// The lowest threshold (0.5 to 0.99) at which every calibrated acting
/// answer at or above it was right in `scored`, if any.
pub fn fit_threshold(scored: &[Scored], temperature: f64, act: &[&str]) -> Option<f64> {
    let acting: Vec<(f64, bool)> = scored
        .iter()
        .filter(|s| s.predicted.as_deref().is_some_and(|p| act.contains(&p)))
        .filter_map(|s| Some((scale(s.confidence?, temperature), s.correct())))
        .collect();
    (50..=99).map(|step| step as f64 / 100.0).find(|threshold| {
        let above: Vec<&(f64, bool)> = acting.iter().filter(|(c, _)| c >= threshold).collect();
        !above.is_empty() && above.iter().all(|(_, ok)| *ok)
    })
}

/// The `settings.toml` block that loads a fit into the engine.
pub fn calibration_toml(question: &str, temperature: f64, threshold: Option<f64>) -> String {
    let mut block = format!("[decisions.calibration.{question}]\ntemperature = {temperature:.1}\n");
    if let Some(threshold) = threshold {
        block.push_str(&format!("threshold = {threshold:.2}\n"));
    }
    block
}

/// `question=temperature[:threshold],...` from `ZENGINE_DECISIONS_CALIBRATION`.
pub fn parse_calibration(text: &str) -> Vec<(String, f64, Option<f64>)> {
    text.split(',')
        .filter_map(|entry| {
            let (name, values) = entry.trim().split_once('=')?;
            let (temperature, threshold) = match values.split_once(':') {
                Some((t, th)) => (t, Some(th.trim().parse().ok()?)),
                None => (values, None),
            };
            Some((
                name.trim().to_string(),
                temperature.trim().parse().ok()?,
                threshold,
            ))
        })
        .collect()
}
