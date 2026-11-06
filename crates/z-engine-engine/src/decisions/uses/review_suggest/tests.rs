//! Review suggestions are cards only: the agent is never sent back, path
//! rules suggest without the model, ambiguous files need a confident yes,
//! shadow shows nothing, and each risky file or dismissal counts once.

use serde_json::json;
use z_engine_config::FeatureMode;
use z_engine_decisions::DecisionRecord;
use z_engine_protocol::decisions::SuggestionKind;
use z_engine_protocol::{CallId, ContentBlock, Event, Message, Role};

use super::suggest::{QUESTION, RULES};
use super::*;
use crate::decisions::resolve_suggestion;
use crate::decisions::seams::review_stop;
use crate::decisions::uses::scripted::{Events, Scripted, install, main_run, session, traced};
use crate::run::RunContext;

const FEATURE: FeatureId = FeatureId::DecisionsReviewSuggest;

/// `(suggestion id, areas, paths)` of every review card shown.
fn cards(events: &Events) -> Vec<(String, Vec<String>, Vec<String>)> {
    let events = events.lock().unwrap();
    let shown = events.iter().filter_map(|event| match event {
        Event::Suggested { suggestion } => match &suggestion.kind {
            SuggestionKind::Review { areas, paths } => Some((
                suggestion.suggestion_id.clone(),
                areas.clone(),
                paths.clone(),
            )),
            _ => None,
        },
        _ => None,
    });
    shown.collect()
}

async fn stop(ctx: &RunContext, paths: &[&str]) -> Option<String> {
    let changed: Vec<PathBuf> = paths.iter().map(|path| ctx.core.root.join(path)).collect();
    review_stop(ctx, &changed).await
}

#[tokio::test]
async fn path_rules_suggest_a_review_without_the_model_and_once_per_file() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, Scripted::down());
    let changed = [
        "src/auth/login.rs",
        ".github/workflows/ci.yml",
        "src/parser.rs",
    ];
    assert_eq!(
        stop(&ctx, &changed).await,
        None,
        "the agent is never sent back"
    );
    let shown = cards(&events);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].1, ["authentication", "CI configuration"]);
    assert_eq!(
        shown[0].2,
        ["src/auth/login.rs", ".github/workflows/ci.yml"]
    );
    let ruled = |record: &DecisionRecord| record.question == RULES;
    assert!(traced(&handle, ruled).await);
    assert_eq!(stop(&ctx, &changed).await, None);
    assert_eq!(cards(&events).len(), 1, "each risky file is suggested once");
    handle.close("test").await;
}

#[tokio::test]
async fn ambiguous_files_need_a_confident_yes() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    for model in [
        Scripted::default().yes(QUESTION, false),
        Scripted::down(),
        Scripted::default(),
    ] {
        install(&handle, FEATURE, FeatureMode::On, model);
        stop(&ctx, &["src/session/store.rs", "README.md"]).await;
    }
    assert!(cards(&events).is_empty());
    install(
        &handle,
        FEATURE,
        FeatureMode::On,
        Scripted::default().yes(QUESTION, true),
    );
    stop(&ctx, &["src/session/store.rs", "README.md"]).await;
    let shown = cards(&events);
    assert_eq!(shown[0].1, ["security-sensitive files"]);
    assert_eq!(shown[0].2, ["src/session/store.rs"]);
    handle.close("test").await;
}

#[tokio::test]
async fn off_and_shadow_show_nothing_and_a_dismissal_ends_the_suggestions() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::Off, Scripted::down());
    stop(&ctx, &["src/auth/login.rs"]).await;
    assert!(handle.core.decisions.trace().recent(10).is_empty());
    install(&handle, FEATURE, FeatureMode::Shadow, Scripted::down());
    stop(&ctx, &["src/auth/login.rs"]).await;
    let would = |record: &DecisionRecord| record.shadow && record.outcome == "suggested review";
    assert!(
        traced(&handle, would).await,
        "shadow records what it would suggest"
    );
    assert!(cards(&events).is_empty());
    install(&handle, FEATURE, FeatureMode::On, Scripted::down());
    stop(&ctx, &["db/migrations/0001.sql"]).await;
    let id = cards(&events).pop().unwrap().0;
    resolve_suggestion(&handle.core, &id, false);
    stop(&ctx, &["src/oauth.rs"]).await;
    assert_eq!(
        cards(&events).len(),
        1,
        "no more suggestions after a dismissal"
    );
    handle.close("test").await;
}

#[tokio::test]
async fn shell_deletions_of_the_turn_are_a_risky_area() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    let rm = ContentBlock::ToolUse {
        id: CallId::from("c1"),
        name: "Bash".into(),
        input: json!({ "command": "rm -rf legacy/" }),
    };
    handle.core.with_state(|state| {
        state
            .working
            .push(Message::user_text("drop the legacy module"));
        state.working.push(Message::new(Role::Assistant, vec![rm]));
        let result = ContentBlock::tool_result(CallId::from("c1"), "", false);
        state.working.push(Message::new(Role::User, vec![result]));
    });
    install(&handle, FEATURE, FeatureMode::On, Scripted::down());
    stop(&ctx, &[]).await;
    let shown = cards(&events);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].1, ["deleted files"]);
    assert!(shown[0].2.is_empty());
    handle.close("test").await;
}
