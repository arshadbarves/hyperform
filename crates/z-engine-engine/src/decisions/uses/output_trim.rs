//! Output trimming (`decisions_output_trim`): long Bash and background-job
//! output is cut into 40-line windows by the tool, and this use asks
//! whether each middle window matters for the user's request. The tool
//! keeps the head, the tail and every window not confidently ruled out,
//! and still saves the full output; no answer keeps today's cut.

use async_trait::async_trait;
use z_engine_config::FeatureId;
use z_engine_prompts::decisions::OUTPUT_TRIM_RELEVANT;
use z_engine_tools::{RankRequest, RankTarget};

use super::relevance::{ask_each, tokens_in};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

pub(super) const QUESTION: &str = "output_trim_relevant";

#[derive(Debug)]
pub(crate) struct OutputTrim;

pub(crate) static OUTPUT_TRIM: OutputTrim = OutputTrim;

#[async_trait]
impl DecisionUse for OutputTrim {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsOutputTrim
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::Relevance]
    }

    async fn relevance(&self, cx: &UseContext, request: &RankRequest) -> Option<Vec<Option<bool>>> {
        if request.target != RankTarget::OutputWindows || request.items.is_empty() {
            return None;
        }
        let ranked = ask_each(cx, request, QUESTION, OUTPUT_TRIM_RELEVANT, "part").await?;
        let (out, total) = (ranked.ruled_out(), request.items.len());
        let saved = tokens_in(request.chars * out / total);
        let outcome = match (out, cx.shadow) {
            (0, _) => "kept every part".to_string(),
            (_, true) => format!("would leave out {out} of {total} parts"),
            (_, false) => format!("left out {out} of {total} parts"),
        };
        ranked.record(cx, &outcome, saved);
        ranked.advice()
    }
}

#[cfg(test)]
mod tests {
    use z_engine_config::FeatureMode;
    use z_engine_decisions::DecisionRecord;

    use super::*;
    use crate::decisions::seams::rank_items;
    use crate::decisions::uses::scripted::{Scripted, install, main_run, session, traced};

    const FEATURE: FeatureId = FeatureId::DecisionsOutputTrim;

    fn windows() -> RankRequest {
        RankRequest {
            target: RankTarget::OutputWindows,
            tool: "Bash".into(),
            subject: "cargo test".into(),
            items: vec!["test a ... ok".into(), "error[E0308]: mismatched".into()],
            chars: 8_000,
        }
    }

    #[tokio::test]
    async fn on_rules_out_windows_and_records_the_tokens_saved() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, _) = session(dir.path()).await;
        let ctx = main_run(&handle);
        install(
            &handle,
            FEATURE,
            FeatureMode::On,
            Scripted::default().yes(QUESTION, false),
        );
        let answers = rank_items(&ctx, windows()).await;
        assert_eq!(answers, Some(vec![Some(false), Some(false)]));
        let records = handle.core.decisions.trace().recent(10);
        assert_eq!(records.len(), 1, "one record per ranking");
        assert_eq!(records[0].outcome, "left out 2 of 2 parts");
        assert_eq!(records[0].tokens_saved, Some(2_000));
        let search = RankRequest {
            target: RankTarget::SearchHits,
            ..windows()
        };
        assert_eq!(rank_items(&ctx, search).await, None, "not its target");
        handle.close("test").await;
    }

    #[tokio::test]
    async fn off_down_unsure_and_shadow_give_no_advice() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, _) = session(dir.path()).await;
        let ctx = main_run(&handle);
        let ruled_out = || Scripted::default().yes(QUESTION, false);
        install(&handle, FEATURE, FeatureMode::Off, ruled_out());
        assert_eq!(rank_items(&ctx, windows()).await, None);
        assert!(handle.core.decisions.trace().recent(10).is_empty());
        for model in [Scripted::down(), Scripted::default()] {
            install(&handle, FEATURE, FeatureMode::On, model);
            assert_eq!(rank_items(&ctx, windows()).await, None);
        }
        install(&handle, FEATURE, FeatureMode::Shadow, ruled_out());
        assert_eq!(rank_items(&ctx, windows()).await, None);
        let would = |r: &DecisionRecord| r.shadow && r.outcome == "would leave out 2 of 2 parts";
        assert!(
            traced(&handle, would).await,
            "shadow records what it would do"
        );
        handle.close("test").await;
    }
}
