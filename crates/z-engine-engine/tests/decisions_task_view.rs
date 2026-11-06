//! `decisions_task_view` end to end: at a new task the main model stops
//! seeing exchanges the task does not need, while standing rules, needed
//! and recent exchanges stay; requests within the task only append; the
//! view survives a reopen; "Include full history" brings everything back.

mod laya;
mod support;

use serde_json::{Value, json};
use support::{BASE_SETTINGS, Harness, all_text};
use z_engine_llm::ModelRequest;
use z_engine_protocol::{Command, Event};
use z_engine_testkit::{FixtureRepo, Script};

/// No cheaper cache reads: the prompt cache never argues against a view.
const UNCACHED: &str = "\n[pricing.\"test-model\"]\ninput = 1.0\noutput = 1.0\n";

fn rule(name: &str, state: &Value) -> &'static str {
    let has = |value: &Value, needle: &str| value.as_str().unwrap_or_default().contains(needle);
    match name {
        "task_boundary" if has(&state["new_message"], "switch") => "unrelated",
        "task_boundary" => "continue",
        "exchange_needed" if has(&state["exchange"]["request"], "keep") => laya::YES,
        "standing_rule" if has(&state["message"], "always") => laya::YES,
        _ => laya::NO,
    }
}

const READS: [(&str, &str); 5] = [
    ("a.txt", "always answer briefly; read a.txt"),
    ("b.txt", "read b.txt"),
    ("c.txt", "keep this one: read c.txt"),
    ("d.txt", "read d.txt"),
    ("e.txt", "read e.txt"),
];

async fn chat() -> Harness {
    let endpoint = laya::laya(rule).await;
    let base = format!("{BASE_SETTINGS}{UNCACHED}");
    let settings = laya::settings(&base, &endpoint, &["decisions_task_view"]);
    let files: Vec<(String, String)> = READS
        .iter()
        .map(|(file, _)| {
            let filler = "filler text that makes each exchange worth setting aside\n".repeat(80);
            (file.to_string(), format!("contents of {file}\n{filler}"))
        })
        .collect();
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(f, c)| (f.as_str(), c.as_str()))
        .collect();
    let mut h = Harness::builder(FixtureRepo::git(&refs))
        .settings(&settings)
        .start()
        .await;
    for (file, prompt) in READS {
        h.model
            .push(Script::tool("Read", json!({ "file_path": h.path(file) })));
        h.model.push(Script::text(&format!("Read {file}.")));
        h.run_turn(prompt).await;
    }
    h
}

fn text(request: &ModelRequest) -> String {
    request
        .messages
        .iter()
        .map(all_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn ids(request: &ModelRequest) -> Vec<String> {
    let rendered = request
        .messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }));
    rendered.map(|m| m.to_string()).collect()
}

#[tokio::test]
async fn a_new_task_sets_aside_what_it_does_not_need() {
    let mut h = chat().await;
    let before = h.main_requests().len();
    assert!(
        text(&h.main_requests()[before - 1]).contains("contents of a.txt"),
        "no view while continuing"
    );

    h.model.push(Script::text("Here is the summary."));
    h.run_turn("switch: summarize the README").await;
    h.expect(|e| matches!(e, Event::TaskViewApplied { view } if !view.restored))
        .await;
    let first = h.main_requests().pop().unwrap();
    let seen = text(&first);
    assert!(
        !seen.contains("contents of a.txt") && !seen.contains("contents of b.txt"),
        "{seen}"
    );
    for kept in [
        "contents of c.txt",
        "contents of d.txt",
        "contents of e.txt",
        "always answer briefly",
    ] {
        assert!(seen.contains(kept), "{kept} must stay");
    }
    assert!(seen.contains("Earlier exchanges of this chat were set aside"));

    h.model.push(Script::text("More detail."));
    h.run_turn("add more detail").await;
    let second = h.main_requests().pop().unwrap();
    assert_eq!(
        ids(&second)[..first.messages.len()],
        ids(&first)[..],
        "within a task requests only append"
    );

    let full = h.transcript();
    assert!(
        full.iter()
            .map(all_text)
            .any(|t| t.contains("contents of a.txt")),
        "the record keeps everything"
    );

    h.reopen().await;
    h.model.push(Script::text("Still here."));
    h.run_turn("and a title").await;
    let reopened = h.main_requests().pop().unwrap();
    assert_eq!(
        ids(&reopened)[..second.messages.len()],
        ids(&second)[..],
        "replay rebuilds the same view"
    );
}

#[tokio::test]
async fn including_full_history_brings_everything_back() {
    let mut h = chat().await;
    h.model.push(Script::text("Here is the summary."));
    h.run_turn("switch: summarize the README").await;
    h.send(Command::IncludeFullHistory);
    h.expect(|e| matches!(e, Event::TaskViewApplied { view } if view.restored))
        .await;
    h.model.push(Script::text("With everything."));
    h.run_turn("now use the earlier reads").await;
    let seen = text(&h.main_requests().pop().unwrap());
    assert!(seen.contains("contents of a.txt") && seen.contains("contents of b.txt"));
    assert!(!seen.contains("Earlier exchanges of this chat were set aside"));
}
