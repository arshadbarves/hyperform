//! Prefetch: confident picks are attached as reminders and tracked as read;
//! off, down, unsure and shadow attach nothing; files the policy does not
//! allow, files already read and files over the budget are never attached;
//! the turn-end record measures tool rounds before the first edit.

use serde_json::json;
use z_engine_config::{FeatureId, FeatureMode};
use z_engine_context::{render_template, wrap_reminder};
use z_engine_decisions::DecisionRecord;
use z_engine_policy::{Policy, PolicyConfig};
use z_engine_prompts::reminders::PREFETCHED_FILE;
use z_engine_protocol::{CallId, ContentBlock, Message, Role};

use super::attach::read_within;
use super::candidates::{Candidate, NAMED};
use super::metrics::measure;
use super::pick::QUESTION;
use crate::decisions::seams::at_turn_start;
use crate::decisions::uses::scripted::{Scripted, install, main_run, session, traced};
use crate::session::SessionHandle;
use crate::sync::lock;

const FEATURE: FeatureId = FeatureId::DecisionsPrefetch;
const REQUEST: &str = "Please fix the bug in src/parser.rs today";

fn write(handle: &SessionHandle, path: &str, content: &str) {
    let full = handle.core.root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, content).unwrap();
}

fn needed() -> Scripted {
    Scripted::default().yes(QUESTION, true)
}

#[tokio::test]
async fn confident_picks_are_attached_and_tracked_as_read() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    write(&handle, "src/parser.rs", "fn parse() {}\n");
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, needed());
    at_turn_start(&ctx, REQUEST).await;
    let notes = handle.core.reminders.take(&ctx.spec.agent_id);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("src/parser.rs") && notes[0].contains("fn parse() {}"));
    let read = handle.core.root.join("src/parser.rs");
    assert!(handle.core.main.files.was_read(&read));
    let records = handle.core.decisions.trace().recent(10);
    assert!(
        records
            .iter()
            .any(|r| r.outcome.starts_with("attached 1 file ("))
    );
    at_turn_start(&ctx, REQUEST).await;
    assert!(
        handle.core.reminders.take(&ctx.spec.agent_id).is_empty(),
        "already read"
    );
    handle.close("test").await;
}

#[tokio::test]
async fn off_down_unsure_short_and_shadow_attach_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    write(&handle, "src/parser.rs", "fn parse() {}\n");
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::Off, needed());
    at_turn_start(&ctx, REQUEST).await;
    for model in [Scripted::down(), Scripted::default()] {
        install(&handle, FEATURE, FeatureMode::On, model);
        at_turn_start(&ctx, REQUEST).await;
    }
    install(&handle, FEATURE, FeatureMode::On, needed());
    at_turn_start(&ctx, "fix src/parser.rs").await;
    install(&handle, FEATURE, FeatureMode::Shadow, needed());
    at_turn_start(&ctx, REQUEST).await;
    let would = |r: &DecisionRecord| r.shadow && r.outcome.starts_with("would attach 1 file");
    assert!(
        traced(&handle, would).await,
        "shadow records what it would do"
    );
    assert!(handle.core.reminders.take(&ctx.spec.agent_id).is_empty());
    assert!(
        !handle
            .core
            .main
            .files
            .was_read(&handle.core.root.join("src/parser.rs"))
    );
    handle.close("test").await;
}

#[tokio::test]
async fn files_the_policy_does_not_allow_are_never_attached() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    for path in ["src/secret.rs", "src/ask.rs", "src/open.rs"] {
        write(&handle, path, "fn x() {}\n");
    }
    let config = PolicyConfig {
        deny: vec!["Read(src/secret.rs)".into()],
        ask: vec!["Read(src/ask.rs)".into()],
        ..PolicyConfig::default()
    };
    let context = lock(&handle.core.policy).context().clone();
    *lock(&handle.core.policy) = Policy::new(&config, context).0;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, needed());
    at_turn_start(
        &ctx,
        "Compare src/secret.rs, src/ask.rs and src/open.rs please",
    )
    .await;
    let notes = handle.core.reminders.take(&ctx.spec.agent_id);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("src/open.rs"));
    handle.close("test").await;
}

#[tokio::test]
async fn the_budget_caps_files_and_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    write(&handle, "big.rs", &"let x = 1;\n".repeat(400));
    write(&handle, "small.rs", "fn a() {}\n");
    write(&handle, "tiny.rs", "fn b() {}\n");
    let pick = |path: &str| Candidate {
        path: path.into(),
        why: NAMED,
        defines: Vec::new(),
    };
    let picks = [pick("big.rs"), pick("small.rs"), pick("tiny.rs")];
    let one = read_within(&handle.core, &picks, 1, 50_000).await;
    assert_eq!(one.len(), 1);
    let small = read_within(&handle.core, &picks, 3, 20).await;
    let names: Vec<_> = small
        .iter()
        .map(|file| file.full.file_name().unwrap())
        .collect();
    assert_eq!(names, ["small.rs", "tiny.rs"], "the big file does not fit");
    handle.close("test").await;
}

#[test]
fn the_measure_counts_rounds_before_the_first_edit_and_prefetched_files() {
    let note = render_template(PREFETCHED_FILE, &[("path", "a.rs"), ("body", "fn a() {}")]);
    let opening = Message::new(
        Role::User,
        vec![
            ContentBlock::text("fix a.rs"),
            ContentBlock::text(wrap_reminder(&note)),
        ],
    );
    let call = |id: &str, name: &str| {
        let input = json!({ "file_path": "a.rs" });
        let call = ContentBlock::ToolUse {
            id: CallId::from(id),
            name: name.into(),
            input,
        };
        Message::new(Role::Assistant, vec![call])
    };
    let result = |id: &str| {
        let result = ContentBlock::tool_result(CallId::from(id), "ok", false);
        Message::new(Role::User, vec![result])
    };
    let mut working = vec![
        Message::user_text("old"),
        opening,
        call("c1", "Grep"),
        result("c1"),
    ];
    assert_eq!(measure(&working), (None, 1), "no edit yet");
    working.extend([
        call("c2", "Read"),
        result("c2"),
        call("c3", "Edit"),
        result("c3"),
    ]);
    assert_eq!(measure(&working), (Some(2), 1));
    assert_eq!(measure(&[]), (None, 0));
}
