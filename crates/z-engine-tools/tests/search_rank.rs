//! `decisions_search_rank`: Grep and Glob show the results the decision
//! model calls relevant first when they hit their cap; below the cap, or
//! without an answer, results are as today.

mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use serde_json::json;
use support::{ctx, ctx_with, ok_text, project, recording_spill};
use z_engine_tools::builtin::{GlobTool, GrepTool};
use z_engine_tools::{Ports, RankRequest, RankTarget, RelevancePort, ToolCtx};

struct Ranker {
    /// Items containing this text are relevant; `None` answers nothing.
    wanted: Option<&'static str>,
    asked: Mutex<Vec<RankRequest>>,
}

#[async_trait]
impl RelevancePort for Ranker {
    fn ranks(&self, target: RankTarget) -> bool {
        target == RankTarget::SearchHits
    }

    async fn rank(&self, _ctx: &ToolCtx, request: RankRequest) -> Option<Vec<Option<bool>>> {
        let answers = self.wanted.map(|wanted| {
            request
                .items
                .iter()
                .map(|item| Some(item.contains(wanted)))
                .collect()
        });
        self.asked.lock().unwrap().push(request);
        answers
    }
}

fn ranker(wanted: Option<&'static str>) -> Arc<Ranker> {
    Arc::new(Ranker {
        wanted,
        asked: Mutex::new(Vec::new()),
    })
}

fn ranked_ctx(root: &std::path::Path, port: &Arc<Ranker>) -> ToolCtx {
    let port: Arc<dyn RelevancePort> = Arc::clone(port) as _;
    ctx_with(
        root,
        Ports {
            relevance: Some(port),
            ..Ports::default()
        },
    )
}

fn many_files() -> tempfile::TempDir {
    let files: Vec<(String, &str)> = (0..5)
        .map(|i| (format!("f{i}.txt"), "needle"))
        .chain([("wanted.txt".to_string(), "needle")])
        .collect();
    let refs: Vec<(&str, &str)> = files.iter().map(|(p, c)| (p.as_str(), *c)).collect();
    let dir = project(&refs);
    let oldest = SystemTime::now() - Duration::from_secs(600);
    std::fs::File::options()
        .write(true)
        .open(dir.path().join("wanted.txt"))
        .unwrap()
        .set_modified(oldest)
        .unwrap();
    dir
}

#[tokio::test]
async fn glob_shows_relevant_files_first_when_it_hits_the_cap() {
    let dir = many_files();
    let port = ranker(Some("wanted"));
    let mut ctx = ranked_ctx(dir.path(), &port);
    ctx.limits.glob_limit = 2;
    let text = ok_text(&GlobTool, &ctx, json!({"pattern": "*.txt"})).await;
    assert!(text.starts_with("wanted.txt\n"), "{text}");
    assert_eq!(text.lines().filter(|l| l.ends_with(".txt")).count(), 2);
    assert!(
        text.contains("the most relevant to the task first"),
        "{text}"
    );
    let asked = port.asked.lock().unwrap();
    assert_eq!(asked[0].items.len(), 6, "three times the limit is walked");
    assert_eq!(asked[0].subject, "*.txt");
}

#[tokio::test]
async fn glob_below_the_cap_or_without_an_answer_is_unchanged() {
    let dir = many_files();
    let port = ranker(Some("wanted"));
    let text = ok_text(
        &GlobTool,
        &ranked_ctx(dir.path(), &port),
        json!({"pattern": "*.txt"}),
    )
    .await;
    let today = ok_text(&GlobTool, &ctx(dir.path()), json!({"pattern": "*.txt"})).await;
    assert_eq!(text, today);
    assert!(port.asked.lock().unwrap().is_empty(), "below the cap");

    let silent = ranker(None);
    let mut ranked = ranked_ctx(dir.path(), &silent);
    ranked.limits.glob_limit = 2;
    let mut plain = ctx(dir.path());
    plain.limits.glob_limit = 2;
    let text = ok_text(&GlobTool, &ranked, json!({"pattern": "*.txt"})).await;
    let today = ok_text(&GlobTool, &plain, json!({"pattern": "*.txt"})).await;
    assert_eq!(text, today);
    assert_eq!(silent.asked.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn grep_over_the_budget_shows_relevant_results_first_and_spills() {
    let dir = many_files();
    let port = ranker(Some("wanted"));
    let mut ctx = ranked_ctx(dir.path(), &port);
    let (spill, spilled) = recording_spill();
    ctx.spill = Some(spill);
    ctx.limits.max_result_chars = 30;
    let text = ok_text(&GrepTool, &ctx, json!({"pattern": "needle"})).await;
    assert!(text.starts_with("wanted.txt\n"), "{text}");
    assert!(text.contains("results shown, the most relevant"), "{text}");
    assert!(text.contains("/artifacts/full-output.txt"), "{text}");
    assert_eq!(spilled.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn grep_pages_and_small_results_are_never_ranked() {
    let dir = many_files();
    let port = ranker(Some("wanted"));
    let mut ranked = ranked_ctx(dir.path(), &port);
    ranked.limits.max_result_chars = 30;
    let mut plain = ctx(dir.path());
    plain.limits.max_result_chars = 30;
    let paged = json!({"pattern": "needle", "head_limit": 3});
    assert_eq!(
        ok_text(&GrepTool, &ranked, paged.clone()).await,
        ok_text(&GrepTool, &plain, paged).await
    );
    let text = ok_text(
        &GrepTool,
        &ranked_ctx(dir.path(), &port),
        json!({"pattern": "needle"}),
    )
    .await;
    assert_eq!(
        text,
        ok_text(&GrepTool, &ctx(dir.path()), json!({"pattern": "needle"})).await
    );
    assert!(port.asked.lock().unwrap().is_empty());
}
