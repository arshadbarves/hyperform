//! Every decision feature in shadow against a confident decision model that
//! says yes to everything: the scripted session runs exactly as with every
//! feature off (same main requests, approvals, tool outcomes and
//! verification), while the decision trace records what each feature
//! would have done, marked as shadow.

mod laya;
mod support;

use std::collections::BTreeSet;

use laya::workout::{self, BASE};
use serde_json::Value;

fn yes(_name: &str, _state: &Value) -> &'static str {
    laya::YES
}

#[tokio::test]
async fn shadow_records_and_never_acts() {
    let mut off = workout::run(BASE, false).await;
    let endpoint = laya::laya(yes).await;
    let settings = laya::every_feature(BASE, &endpoint, "shadow", laya::ANSWER_TIMEOUT_MS);
    let mut shadow = workout::run(&settings, false).await;

    assert_eq!(shadow.verification(), off.verification());
    assert_eq!(shadow.asked(), off.asked());
    assert_eq!(shadow.statuses(), off.statuses());
    let (off_requests, shadow_requests) = (off.requests(), shadow.requests());
    assert_eq!(off_requests.len(), shadow_requests.len());
    for (index, (seen, expected)) in shadow_requests.iter().zip(&off_requests).enumerate() {
        assert_eq!(seen, expected, "main request {index}");
    }

    let h = &shadow.h;
    let decisions = h.engine.session_decisions(&h.session, 1000).unwrap();
    assert!(!decisions.records.is_empty(), "shadow leaves a trace");
    assert!(decisions.records.iter().all(|record| record.shadow));
    let features: BTreeSet<&str> = decisions
        .records
        .iter()
        .map(|r| r.feature.as_str())
        .collect();
    assert!(features.len() >= 5, "{features:?}");
}
