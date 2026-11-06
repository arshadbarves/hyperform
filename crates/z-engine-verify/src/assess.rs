//! The per-turn verification badge computed from check records. It is
//! evidence for the user and the model, never a gate on finishing.

use z_engine_protocol::{CheckKind, CheckRecord, VerificationOutcome};

/// Failing checks named in a `Failed` reason before the rest are counted.
const MAX_NAMED_FAILURES: usize = 3;

/// The outcome for a turn. `mutated` says whether files changed;
/// `last_mutation_at` (epoch ms) limits evidence to checks started at or
/// after the last change (all records when `None`); `current_fingerprint`
/// (the workspace now) makes records whose `fingerprint_after` differs
/// stale. Only the latest record of each check counts.
pub fn assess(
    records: &[CheckRecord],
    mutated: bool,
    last_mutation_at: Option<u64>,
    current_fingerprint: Option<&str>,
) -> VerificationOutcome {
    if !mutated {
        return VerificationOutcome::NotApplicable;
    }
    let latest = latest_per_check(
        records
            .iter()
            .filter(|r| last_mutation_at.is_none_or(|at| r.started_at >= at)),
    );
    if latest.is_empty() {
        return unverified("no checks have run since the last change");
    }
    let failing: Vec<&CheckRecord> = latest.iter().copied().filter(|r| !r.passed).collect();
    if !failing.is_empty() {
        return VerificationOutcome::Failed {
            reason: failure_reason(&failing),
        };
    }
    let fresh: Vec<&CheckRecord> = latest
        .iter()
        .copied()
        .filter(|r| current_fingerprint.is_none_or(|now| r.fingerprint_after == now))
        .collect();
    if fresh.is_empty() {
        return unverified("checks are stale: files changed after they ran");
    }
    let substantive = fresh.iter().any(|r| {
        matches!(
            r.kind,
            CheckKind::Test | CheckKind::Build | CheckKind::Typecheck
        )
    });
    if !substantive {
        return unverified("only lint/format checks ran; run tests or a build");
    }
    VerificationOutcome::Verified {
        checks: fresh.iter().map(|r| r.record_id.clone()).collect(),
    }
}

fn unverified(reason: &str) -> VerificationOutcome {
    VerificationOutcome::Unverified {
        reason: reason.to_string(),
    }
}

/// The newest record per `check_id` (a later record wins a tie), in order
/// of each check's first appearance.
fn latest_per_check<'a>(records: impl Iterator<Item = &'a CheckRecord>) -> Vec<&'a CheckRecord> {
    let mut latest: Vec<&CheckRecord> = Vec::new();
    for record in records {
        match latest.iter_mut().find(|r| r.check_id == record.check_id) {
            Some(slot) if record.started_at >= slot.started_at => *slot = record,
            Some(_) => {}
            None => latest.push(record),
        }
    }
    latest
}

fn failure_reason(failing: &[&CheckRecord]) -> String {
    let mut parts: Vec<String> = failing
        .iter()
        .take(MAX_NAMED_FAILURES)
        .map(|r| format!("{} failed ({})", name(r), detail(r)))
        .collect();
    if failing.len() > MAX_NAMED_FAILURES {
        parts.push(format!(
            "{} more failed",
            failing.len() - MAX_NAMED_FAILURES
        ));
    }
    parts.join("; ")
}

fn name(record: &CheckRecord) -> &str {
    if record.label.trim().is_empty() {
        &record.check_id
    } else {
        &record.label
    }
}

fn detail(record: &CheckRecord) -> String {
    let mut detail = match (record.timed_out, record.exit_code) {
        (true, _) => "timed out".to_string(),
        (false, Some(code)) => format!("exit {code}"),
        (false, None) => "killed".to_string(),
    };
    if let Some(tests) = record.tests.filter(|t| t.failed > 0) {
        let noun = if tests.failed == 1 { "test" } else { "tests" };
        detail.push_str(&format!(", {} {noun} failed", tests.failed));
    }
    detail
}

#[cfg(test)]
#[path = "assess_tests.rs"]
mod tests;
