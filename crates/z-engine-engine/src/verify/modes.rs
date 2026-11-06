//! Verification modes at the main agent's stop boundary, for turns that
//! changed files. `report` only computes the badge. `auto` runs the
//! selected checks (trusted workspaces only) when no fresh evidence exists
//! and feeds failures back; `strict` also keeps the turn going until the
//! badge is `Verified`. Both are bounded by `max_continuations`. Decision
//! uses may narrow the selection and flag claims no check backs.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use z_engine_context::{auto_check_failed, verification_required};
use z_engine_protocol::{AgentId, CheckRecord, NoticeLevel, VerificationMode, VerificationOutcome};
use z_engine_verify::select_checks;

use super::claims::{claim_checks, show, unchecked_claim};
use super::outcome::current_outcome;
use super::record::{CheckRun, run_recorded};
use super::verdict::{StopVerdict, Verifier};
use crate::decisions::seams::select_needed;
use crate::run::RunContext;

const UNTRUSTED: &str = "automatic checks are disabled for untrusted projects";
/// Output lines of a failed check quoted back to the model.
const FAILURE_TAIL_LINES: usize = 40;

#[derive(Debug, Default)]
pub(crate) struct ModeVerifier;

/// What the harness did before judging the turn.
enum AutoRun {
    /// Nothing ran; the reason explains an unverified badge.
    Skipped(&'static str),
    /// Every selected check ran; failures are summarized.
    Ran { failures: Option<String> },
}

#[async_trait]
impl Verifier for ModeVerifier {
    async fn at_stop(
        &self,
        ctx: &RunContext,
        continuations: u32,
        changed: &[PathBuf],
    ) -> StopVerdict {
        let core = &ctx.core;
        let settings = core.settings();
        let verification = &settings.settings.verification;
        let mode = verification.mode;
        let mutated = core.with_state(|state| state.mutation.mutated);
        let mut outcome = current_outcome(core).await;
        if !mutated || !matches!(mode, VerificationMode::Auto | VerificationMode::Strict) {
            if mode == VerificationMode::Report
                && let Some(claim) = unchecked_claim(ctx, &outcome, changed).await
            {
                show(ctx, claim);
            }
            return StopVerdict::Done(outcome);
        }
        let mut failures = None;
        if matches!(outcome, VerificationOutcome::Unverified { .. }) {
            let mut run = auto_run(ctx, changed, &verification.auto_checks).await;
            if let AutoRun::Skipped(reason) = run
                && let Some(claim) = unchecked_claim(ctx, &outcome, changed).await
            {
                if reason != UNTRUSTED {
                    run = auto_run(ctx, changed, &claim_checks()).await;
                }
                if matches!(run, AutoRun::Skipped(_)) {
                    show(ctx, claim);
                }
            }
            match run {
                AutoRun::Skipped(reason) => outcome = explained(outcome, reason),
                AutoRun::Ran { failures: failed } => {
                    failures = failed;
                    outcome = current_outcome(core).await;
                }
            }
        }
        if matches!(outcome, VerificationOutcome::Verified { .. }) {
            return StopVerdict::Done(outcome);
        }
        let budget = verification.max_continuations;
        if continuations >= budget {
            if continuations > 0 {
                core.events.notice(
                    NoticeLevel::Warn,
                    format!(
                        "Verification ended after {continuations} automatic continuation(s); \
                         the changes are {}.",
                        outcome.label()
                    ),
                );
            }
            return StopVerdict::Done(outcome);
        }
        match (failures, mode) {
            (Some(summary), _) => StopVerdict::Continue {
                reminder: auto_check_failed(&summary),
            },
            (None, VerificationMode::Strict) => StopVerdict::Continue {
                reminder: verification_required(&outcome, mode),
            },
            (None, _) => StopVerdict::Done(outcome),
        }
    }
}

/// Runs the checks `selectors` pick for the changed paths as the main
/// agent, less those the check-selection use finds unaffected. A cancelled
/// or broken run stops early; its reason is logged.
async fn auto_run(ctx: &RunContext, changed: &[PathBuf], selectors: &[String]) -> AutoRun {
    let core = &ctx.core;
    let settings = core.settings();
    if !settings.trusted {
        return AutoRun::Skipped(UNTRUSTED);
    }
    let profile = core.checks.profile();
    let relative: Vec<PathBuf> = changed
        .iter()
        .filter_map(|path| path.strip_prefix(&core.root).ok())
        .map(Path::to_path_buf)
        .collect();
    let selected = select_checks(&profile, selectors, &relative);
    if selected.is_empty() {
        return AutoRun::Skipped("no project check matches verification.auto_checks");
    }
    let strict = settings.settings.verification.mode == VerificationMode::Strict;
    let selected = select_needed(ctx, selected, &relative, strict).await;
    if selected.is_empty() {
        return AutoRun::Skipped("no selected check can be affected by these changes");
    }
    let mut failed = Vec::new();
    for spec in selected {
        let run = CheckRun {
            agent_id: AgentId::main(),
            root: core.root.clone(),
            cancel: ctx.cancel.clone(),
            progress: None,
        };
        match run_recorded(core, spec, run).await {
            Ok(record) if !record.passed => failed.push(record),
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "automatic check stopped");
                break;
            }
        }
    }
    AutoRun::Ran {
        failures: (!failed.is_empty()).then(|| summarize(&failed)),
    }
}

fn explained(outcome: VerificationOutcome, reason: &str) -> VerificationOutcome {
    match outcome {
        VerificationOutcome::Unverified { .. } => VerificationOutcome::Unverified {
            reason: reason.to_string(),
        },
        other => other,
    }
}

fn summarize(failed: &[CheckRecord]) -> String {
    failed
        .iter()
        .map(|record| {
            let status = match (record.timed_out, record.exit_code) {
                (true, _) => "timed out".to_string(),
                (false, Some(code)) => format!("exit code {code}"),
                (false, None) => "killed".to_string(),
            };
            let mut text = format!("{} (`{}`): {status}", record.label, record.command);
            if let Some(tests) = record.tests.filter(|tests| tests.failed > 0) {
                text.push_str(&format!(", {} test(s) failed", tests.failed));
            }
            if let Some(artifact) = &record.artifact {
                text.push_str(&format!("\nFull output: {artifact}"));
            }
            let tail = tail_lines(&record.output_tail, FAILURE_TAIL_LINES);
            if !tail.is_empty() {
                text.push_str(&format!("\n```\n{tail}\n```"));
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn tail_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tails_keep_the_last_lines() {
        assert_eq!(tail_lines("a\nb\nc\n", 2), "b\nc");
        assert_eq!(tail_lines("", 3), "");
    }

    #[test]
    fn only_unverified_badges_take_the_skip_reason() {
        let unverified = VerificationOutcome::Unverified {
            reason: "no checks".into(),
        };
        assert_eq!(
            explained(unverified, UNTRUSTED),
            VerificationOutcome::Unverified {
                reason: UNTRUSTED.into()
            }
        );
        let failed = VerificationOutcome::Failed { reason: "x".into() };
        assert_eq!(explained(failed.clone(), UNTRUSTED), failed);
    }
}
