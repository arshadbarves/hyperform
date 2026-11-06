//! Search ranking (`decisions_search_rank`): when Grep or Glob found more
//! than they can show, this use asks whether results are useful for the
//! user's request, and the tool shows the useful ones first so they
//! survive the cut. At most `MAX_ASKED` results are asked about: those
//! sharing the most words with the request, then the earliest. Below the
//! cap tools never ask; no answer keeps today's order.

use std::collections::HashSet;

use async_trait::async_trait;
use z_engine_config::FeatureId;
use z_engine_prompts::decisions::SEARCH_RANK_RELEVANT;
use z_engine_tools::{RankRequest, RankTarget};

use super::digest::latest_request;
use super::relevance::ask_each;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};

pub(super) const QUESTION: &str = "search_rank_relevant";
const MAX_ASKED: usize = 24;

#[derive(Debug)]
pub(crate) struct SearchRank;

pub(crate) static SEARCH_RANK: SearchRank = SearchRank;

#[async_trait]
impl DecisionUse for SearchRank {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsSearchRank
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::Relevance]
    }

    async fn relevance(&self, cx: &UseContext, request: &RankRequest) -> Option<Vec<Option<bool>>> {
        if request.target != RankTarget::SearchHits || request.items.is_empty() {
            return None;
        }
        let task = cx.core.with_state(|state| latest_request(&state.working));
        let picked = pick(&task, &request.items, MAX_ASKED);
        let asked = RankRequest {
            items: picked.iter().map(|&i| request.items[i].clone()).collect(),
            ..request.clone()
        };
        let ranked = ask_each(cx, &asked, QUESTION, SEARCH_RANK_RELEVANT, "result").await?;
        let (useful, total) = (ranked.relevant(), request.items.len());
        let outcome = match (useful, cx.shadow) {
            (0, _) => "kept the usual order".to_string(),
            (_, true) => format!("would show {useful} of {total} results first"),
            (_, false) => format!("showed {useful} of {total} results first"),
        };
        ranked.record(cx, &outcome, 0);
        let mut verdicts = vec![None; total];
        for (&index, verdict) in picked.iter().zip(ranked.advice()?) {
            verdicts[index] = verdict;
        }
        Some(verdicts)
    }
}

/// Indices of at most `limit` items, in result order: those sharing the
/// most words with `task`, ties to the earliest.
fn pick(task: &str, items: &[String], limit: usize) -> Vec<usize> {
    let wanted = words(task);
    let overlap = |item: &str| words(item).intersection(&wanted).count();
    let mut scored: Vec<(usize, usize)> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (overlap(item), index))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut picked: Vec<usize> = scored.into_iter().take(limit).map(|(_, i)| i).collect();
    picked.sort_unstable();
    picked
}

/// Lowercase words of three or more letters; paths split on `/`, `_`, `.`.
fn words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.len() >= 3)
        .map(str::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use z_engine_config::FeatureMode;
    use z_engine_decisions::DecisionRecord;

    use super::*;
    use crate::decisions::seams::rank_items;
    use crate::decisions::uses::scripted::{Scripted, install, main_run, session, traced};

    const FEATURE: FeatureId = FeatureId::DecisionsSearchRank;

    fn hits(count: usize) -> RankRequest {
        RankRequest {
            target: RankTarget::SearchHits,
            tool: "Glob".into(),
            subject: "**/*.rs".into(),
            items: (0..count).map(|i| format!("src/file_{i}.rs")).collect(),
            chars: 400,
        }
    }

    #[test]
    fn picks_the_results_sharing_words_with_the_request() {
        let items: Vec<String> = [
            "src/a.rs",
            "src/parser/lexer.rs",
            "README.md",
            "src/parser.rs",
        ]
        .map(String::from)
        .into();
        assert_eq!(pick("fix the parser", &items, 2), [1, 3]);
        assert_eq!(pick("", &items, 2), [0, 1], "no overlap: the earliest");
        assert_eq!(pick("x", &items, 10), [0, 1, 2, 3]);
    }

    #[tokio::test]
    async fn on_answers_for_the_asked_results_only() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, _) = session(dir.path()).await;
        let ctx = main_run(&handle);
        install(
            &handle,
            FEATURE,
            FeatureMode::On,
            Scripted::default().yes(QUESTION, true),
        );
        let answers = rank_items(&ctx, hits(30)).await.unwrap();
        assert_eq!(answers.len(), 30);
        assert_eq!(answers.iter().filter(|a| a.is_some()).count(), MAX_ASKED);
        let records = handle.core.decisions.trace().recent(10);
        assert_eq!(records[0].outcome, "showed 24 of 30 results first");
        let windows = RankRequest {
            target: RankTarget::OutputWindows,
            ..hits(3)
        };
        assert_eq!(rank_items(&ctx, windows).await, None, "not its target");
        handle.close("test").await;
    }

    #[tokio::test]
    async fn off_down_unsure_and_shadow_keep_the_order() {
        let dir = tempfile::tempdir().unwrap();
        let (handle, _) = session(dir.path()).await;
        let ctx = main_run(&handle);
        let useful = || Scripted::default().yes(QUESTION, true);
        install(&handle, FEATURE, FeatureMode::Off, useful());
        assert_eq!(rank_items(&ctx, hits(3)).await, None);
        for model in [Scripted::down(), Scripted::default()] {
            install(&handle, FEATURE, FeatureMode::On, model);
            assert_eq!(rank_items(&ctx, hits(3)).await, None);
        }
        install(&handle, FEATURE, FeatureMode::Shadow, useful());
        assert_eq!(rank_items(&ctx, hits(3)).await, None);
        let would = |r: &DecisionRecord| r.shadow && r.outcome == "would show 3 of 3 results first";
        assert!(
            traced(&handle, would).await,
            "shadow records what it would do"
        );
        handle.close("test").await;
    }
}
