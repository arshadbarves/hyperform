use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::*;
use crate::ports::{Ports, RelevancePort};

fn lines(count: usize) -> String {
    (1..=count).map(|i| format!("line {i}\n")).collect()
}

/// Answers every window with `answer(index)`; records what it was asked.
struct Fake {
    answer: fn(usize) -> Option<bool>,
    silent: bool,
    asked: Mutex<Vec<RankRequest>>,
}

#[async_trait]
impl RelevancePort for Fake {
    fn ranks(&self, target: RankTarget) -> bool {
        target == RankTarget::OutputWindows
    }

    async fn rank(&self, _ctx: &ToolCtx, request: RankRequest) -> Option<Vec<Option<bool>>> {
        let answers = (0..request.items.len()).map(self.answer).collect();
        self.asked.lock().unwrap().push(request);
        (!self.silent).then_some(answers)
    }
}

fn ctx_with(fake: Option<Arc<Fake>>) -> (ToolCtx, Arc<Mutex<Vec<String>>>) {
    let mut ctx = ToolCtx::for_tests(std::env::temp_dir());
    let spilled = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&spilled);
    ctx.spill = Some(Arc::new(move |_: &str, full: &str| {
        sink.lock().unwrap().push(full.to_string());
        Some(PathBuf::from("/artifacts/out.txt"))
    }));
    ctx.ports = Arc::new(Ports {
        relevance: fake.map(|fake| fake as Arc<dyn RelevancePort>),
        ..Ports::default()
    });
    (ctx, spilled)
}

fn fake(answer: fn(usize) -> Option<bool>, silent: bool) -> Arc<Fake> {
    Arc::new(Fake {
        answer,
        silent,
        asked: Mutex::default(),
    })
}

#[test]
fn windows_cover_the_text_in_forty_line_steps() {
    let text = lines(95);
    let found = windows(&text);
    assert_eq!(found.len(), 3);
    assert!(text[found[1].clone()].starts_with("line 41\n"));
    assert_eq!(found[2].end, text.len());
    assert_eq!(windows(&lines(80)).len(), 2, "no empty trailing window");
}

#[test]
fn many_windows_ask_about_the_error_like_ones() {
    let mut text = lines(40 * 40);
    text = text.replace("line 1201\n", "error: boom\n");
    let windows = windows(&text);
    let asked = candidates(&text, &windows);
    assert_eq!(asked.len(), MAX_ASKED);
    assert!(asked.contains(&30), "the window with the error is asked");
    assert!(asked.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(candidates(&lines(200), &windows_of(200)), vec![1, 2, 3]);
}

fn windows_of(count: usize) -> Vec<Range<usize>> {
    windows(&lines(count))
}

#[test]
fn nothing_left_out_means_no_trim() {
    assert_eq!(keep_mask(5, &[1, 2, 3], &[None, None, None]), None);
    assert_eq!(
        keep_mask(5, &[1, 2, 3], &[Some(true), None, Some(true)]),
        None
    );
    let keep = keep_mask(5, &[1, 2, 3], &[Some(false), None, Some(true)]).unwrap();
    assert_eq!(
        keep,
        [Keep::Yes, Keep::No, Keep::Unsure, Keep::Yes, Keep::Yes]
    );
}

#[test]
fn left_out_runs_become_one_marker_each() {
    let text = lines(240);
    let windows = windows(&text);
    let keep = [
        Keep::Yes,
        Keep::No,
        Keep::No,
        Keep::Yes,
        Keep::No,
        Keep::Yes,
    ];
    let out = assemble(
        &text,
        &windows,
        &keep,
        Some(std::path::Path::new("/a/out.txt")),
    );
    assert!(out.starts_with("line 1\n"));
    assert!(out.contains("line 40\n[... lines 41-120 left out"));
    assert!(out.contains("saved at /a/out.txt"));
    assert!(out.contains(
        "line 160\n[... lines 161-200 left out: not relevant to the current task.]\nline 201\n"
    ));
    assert!(out.ends_with("line 240\n"));
}

#[tokio::test]
async fn without_a_ranker_or_an_answer_output_is_unchanged() {
    let text = lines(400);
    let (plain, _) = ctx_with(None);
    let today = truncate_output(&plain, "bash", &text);
    assert_eq!(
        trim_output(&plain, "bash", "Bash", "seq", &text).await,
        today
    );
    let silent = fake(|_| Some(false), true);
    let (ctx, _) = ctx_with(Some(Arc::clone(&silent)));
    assert_eq!(trim_output(&ctx, "bash", "Bash", "seq", &text).await, today);
    assert_eq!(silent.asked.lock().unwrap()[0].items.len(), 8);
    let unsure = fake(|_| None, false);
    let (ctx, spilled) = ctx_with(Some(unsure));
    assert_eq!(trim_output(&ctx, "bash", "Bash", "seq", &text).await, today);
    assert!(spilled.lock().unwrap().is_empty());
}

#[tokio::test]
async fn short_output_is_never_asked_about() {
    let quiet = fake(|_| Some(false), false);
    let (ctx, _) = ctx_with(Some(Arc::clone(&quiet)));
    let text = lines(120);
    assert_eq!(trim_output(&ctx, "bash", "Bash", "seq", &text).await, text);
    assert!(quiet.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn windows_ruled_out_are_left_out_and_the_full_output_is_spilled() {
    let ranker = fake(|i| Some(i == 2), false);
    let (ctx, spilled) = ctx_with(Some(Arc::clone(&ranker)));
    let text = lines(400);
    let out = trim_output(&ctx, "bash", "Bash", "cargo test", &text).await;
    assert!(out.starts_with("line 1\n") && out.ends_with("line 400\n"));
    assert!(out.contains("line 121\n") && !out.contains("line 41\n"));
    assert!(out.contains("saved at /artifacts/out.txt"), "{out}");
    assert!(out.len() < text.len() / 2);
    assert_eq!(spilled.lock().unwrap().as_slice(), [text]);
    let asked = &ranker.asked.lock().unwrap()[0];
    assert_eq!(
        (asked.tool.as_str(), asked.subject.as_str()),
        ("Bash", "cargo test")
    );
    assert!(asked.items[0].starts_with("line 41\n"));
}

#[tokio::test]
async fn over_budget_drops_unsure_windows_then_falls_back() {
    let unsure_rest = fake(|i| (i == 0).then_some(false), false);
    let (mut ctx, _) = ctx_with(Some(unsure_rest));
    ctx.limits.max_result_chars = 1_500;
    let text = lines(400);
    let out = trim_output(&ctx, "bash", "Bash", "seq", &text).await;
    assert!(out.contains("lines 41-360 left out"), "{out}");
    ctx.limits.max_result_chars = 500;
    let today = truncate_output(&ctx, "bash", &text);
    assert_eq!(trim_output(&ctx, "bash", "Bash", "seq", &text).await, today);
}
