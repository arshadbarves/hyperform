//! `[decisions]`: the decision model the `decisions_*` features ask, and
//! how the engine reaches it.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const DEFAULT_DECISIONS_ENDPOINT: &str = "http://127.0.0.1:8000";
pub const DEFAULT_DECISIONS_CHECKPOINT: &str = "multilingual";
pub const DECISIONS_TIMEOUT_MS: RangeInclusive<u64> = 50..=10_000;
pub const DECISIONS_MAX_LEN: RangeInclusive<u32> = 128..=8_192;
pub const DECISIONS_MAX_BATCH: RangeInclusive<u32> = 1..=64;
const DEFAULT_THRESHOLD: f64 = 0.8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum DecisionRuntime {
    /// A Jev-compatible `POST /v1/systemone` server (laya-serve) at
    /// `endpoint`, optionally started from `sidecar.command`.
    #[default]
    Sidecar,
    /// Laya in-process through ONNX Runtime, from the model downloaded in
    /// Settings. Needs an app built with the `onnx` feature; without it, or
    /// without the model, decisions fall back to rules with a warning.
    Native,
}

/// `[decisions.sidecar]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct DecisionSidecarSettings {
    /// Shell command that starts laya-serve; when set it replaces
    /// `endpoint`. The engine picks a free loopback port and sets
    /// `LAYA_HOST`, `LAYA_PORT`, `LAYA_API_KEY` (random per launch),
    /// `LAYA_CHECKPOINT` and `LAYA_MAX_LEN` for it.
    pub command: Option<String>,
}

/// `[decisions.calibration.<question>]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct CalibrationEntry {
    /// Divides the model's log-probabilities; above 1 softens an
    /// over-confident question.
    pub temperature: f64,
    /// Confidence needed to act on this question; unset uses `threshold`.
    pub threshold: Option<f64>,
}

impl Default for CalibrationEntry {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            threshold: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct DecisionSettings {
    /// Base URL of the decision server; loopback only unless `allow_remote`.
    pub endpoint: String,
    /// Environment variable holding the server's API key (e.g. Jev's).
    pub api_key_env: Option<String>,
    /// Laya checkpoint the sidecar or native runtime loads: `english`,
    /// `multilingual` or `typed-decisions`.
    pub checkpoint: String,
    /// Tokens the checkpoint reads per question; longer inputs are cut.
    pub max_len: u32,
    /// Longest wait for an answer; a slower answer counts as no answer.
    #[ts(type = "number")]
    pub timeout_ms: u64,
    /// Confidence (0 to 1) an answer needs before a feature acts on it.
    pub threshold: f64,
    /// Most questions sent in one request.
    pub max_batch: u32,
    /// Allow an endpoint that is not on this machine.
    pub allow_remote: bool,
    pub runtime: DecisionRuntime,
    pub sidecar: DecisionSidecarSettings,
    /// Save decision inputs and answers to a local dataset folder.
    pub record_dataset: bool,
    /// Question name to its calibration.
    pub calibration: BTreeMap<String, CalibrationEntry>,
    pub prefetch: super::DecisionPrefetchSettings,
    pub routing: super::DecisionRoutingSettings,
    pub loop_guard: super::DecisionLoopGuardSettings,
    pub task_view: super::DecisionTaskViewSettings,
    /// `[[decisions.rules]]`; user and project rules add up.
    pub rules: Vec<super::DecisionRule>,
}

impl Default for DecisionSettings {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_DECISIONS_ENDPOINT.to_string(),
            api_key_env: None,
            checkpoint: DEFAULT_DECISIONS_CHECKPOINT.to_string(),
            max_len: 1_024,
            timeout_ms: 250,
            threshold: DEFAULT_THRESHOLD,
            max_batch: 16,
            allow_remote: false,
            runtime: DecisionRuntime::Sidecar,
            sidecar: DecisionSidecarSettings::default(),
            record_dataset: false,
            calibration: BTreeMap::new(),
            prefetch: Default::default(),
            routing: Default::default(),
            loop_guard: Default::default(),
            task_view: Default::default(),
            rules: Vec::new(),
        }
    }
}

/// Clamps ranges, fills blank strings with defaults, and drops unusable
/// calibration entries, warning about each change.
pub(super) fn normalize_decisions(d: &mut DecisionSettings, w: &mut Vec<String>) {
    let endpoint = d.endpoint.trim().trim_end_matches('/');
    d.endpoint = match endpoint {
        "" => DEFAULT_DECISIONS_ENDPOINT.to_string(),
        url => url.to_string(),
    };
    super::decisions_prefetch::normalize_prefetch(&mut d.prefetch, w);
    super::decisions_rules::normalize_rules(&mut d.rules, w);
    if d.checkpoint.trim().is_empty() {
        d.checkpoint = DEFAULT_DECISIONS_CHECKPOINT.to_string();
    }
    for value in [&mut d.api_key_env, &mut d.sidecar.command] {
        *value = value
            .take()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
    }
    clamp(
        w,
        "decisions.timeout_ms",
        &mut d.timeout_ms,
        DECISIONS_TIMEOUT_MS,
    );
    clamp(w, "decisions.max_len", &mut d.max_len, DECISIONS_MAX_LEN);
    clamp(
        w,
        "decisions.max_batch",
        &mut d.max_batch,
        DECISIONS_MAX_BATCH,
    );
    if !is_probability(d.threshold) {
        w.push(format!(
            "decisions.threshold = {} is out of range; using {DEFAULT_THRESHOLD}",
            d.threshold
        ));
        d.threshold = DEFAULT_THRESHOLD;
    }
    d.calibration.retain(|name, entry| {
        let key = format!("decisions.calibration.{name}");
        if !(entry.temperature.is_finite() && entry.temperature > 0.0) {
            w.push(format!("{key}.temperature must be above 0; entry skipped"));
            return false;
        }
        if entry.threshold.is_some_and(|value| !is_probability(value)) {
            w.push(format!(
                "{key}.threshold must be 0 to 1; using decisions.threshold"
            ));
            entry.threshold = None;
        }
        true
    });
}

fn clamp<T: Ord + Copy + std::fmt::Display>(
    w: &mut Vec<String>,
    key: &str,
    value: &mut T,
    range: RangeInclusive<T>,
) {
    let clamped = (*value).clamp(*range.start(), *range.end());
    if clamped != *value {
        w.push(format!("{key} = {value} is out of range; using {clamped}"));
        *value = clamped;
    }
}

fn is_probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_and_partial_tables_keep_defaults() {
        let parsed: DecisionSettings = toml::from_str(
            "timeout_ms = 400\n[sidecar]\ncommand = \"laya-serve\"\n\
             [calibration.compaction_relevant]\ntemperature = 1.7\n",
        )
        .unwrap();
        assert_eq!(parsed.timeout_ms, 400);
        assert_eq!(parsed.endpoint, DEFAULT_DECISIONS_ENDPOINT);
        assert_eq!(parsed.checkpoint, "multilingual");
        assert!(!parsed.allow_remote && !parsed.record_dataset);
        assert_eq!(parsed.sidecar.command.as_deref(), Some("laya-serve"));
        assert_eq!(parsed.calibration["compaction_relevant"].temperature, 1.7);
        let text = toml::to_string(&parsed).unwrap();
        assert_eq!(toml::from_str::<DecisionSettings>(&text).unwrap(), parsed);
        assert_eq!(parsed.runtime, DecisionRuntime::Sidecar);
        let native: DecisionSettings = toml::from_str("runtime = \"native\"\n").unwrap();
        assert_eq!(native.runtime, DecisionRuntime::Native);
    }

    #[test]
    fn normalize_clamps_and_drops_unusable_calibration() {
        let mut d = DecisionSettings {
            endpoint: " http://127.0.0.1:9000/ ".into(),
            api_key_env: Some("  ".into()),
            timeout_ms: 1,
            threshold: 1.5,
            max_batch: 0,
            ..DecisionSettings::default()
        };
        let bad = CalibrationEntry {
            temperature: 0.0,
            threshold: None,
        };
        let loose = CalibrationEntry {
            temperature: 2.0,
            threshold: Some(-1.0),
        };
        d.calibration.insert("bad".into(), bad);
        d.calibration.insert("loose".into(), loose);
        let mut w = Vec::new();
        normalize_decisions(&mut d, &mut w);
        assert_eq!(d.endpoint, "http://127.0.0.1:9000");
        assert_eq!(d.api_key_env, None);
        assert_eq!((d.timeout_ms, d.max_batch), (50, 1));
        assert_eq!(d.threshold, DEFAULT_THRESHOLD);
        assert!(!d.calibration.contains_key("bad"));
        assert_eq!(d.calibration["loose"].threshold, None);
        assert_eq!(w.len(), 5, "{w:?}");
    }

    #[test]
    fn defaults_need_no_adjustment() {
        let mut d = DecisionSettings::default();
        let mut w = Vec::new();
        normalize_decisions(&mut d, &mut w);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(d, DecisionSettings::default());
    }
}
