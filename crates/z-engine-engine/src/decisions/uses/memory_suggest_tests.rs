//! Standing-rule offers are cards only: nothing is written, off, down,
//! unsure and shadow offer nothing, sentences without a rule marker are
//! never asked about, and a rule is offered once.

use std::sync::Arc;

use z_engine_config::FeatureMode;
use z_engine_context::InstructionDoc;
use z_engine_decisions::DecisionRecord;
use z_engine_protocol::Event;

use super::*;
use crate::decisions::seams::at_turn_start;
use crate::decisions::uses::scripted::{Events, Scripted, install, main_run, session, traced};
use crate::sync::write;

const FEATURE: FeatureId = FeatureId::DecisionsMemorySuggest;
const MESSAGE: &str = "Add a lockfile check. Always use pnpm for installs, never npm.";

fn offered(events: &Events) -> Vec<String> {
    let events = events.lock().unwrap();
    let rules = events.iter().filter_map(|event| match event {
        Event::Suggested { suggestion } => match &suggestion.kind {
            SuggestionKind::SaveRule { rule } => Some(rule.clone()),
            _ => None,
        },
        _ => None,
    });
    rules.collect()
}

fn yes() -> Scripted {
    Scripted::default().yes(QUESTION, true)
}

#[tokio::test]
async fn a_confident_rule_offers_one_card_once_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, yes());
    at_turn_start(&ctx, MESSAGE).await;
    assert_eq!(
        offered(&events),
        ["Always use pnpm for installs, never npm."]
    );
    assert!(handle.core.reminders.take(&ctx.spec.agent_id).is_empty());
    let project = handle.core.root.clone();
    assert!(!project.join("AGENTS.md").exists());
    let records = handle.core.decisions.trace().recent(10);
    assert_eq!(records.len(), 1, "only the marked sentence is asked about");
    at_turn_start(&ctx, MESSAGE).await;
    assert_eq!(offered(&events).len(), 1, "offered once a session");
    handle.close("test").await;
}

#[tokio::test]
async fn no_off_down_unsure_and_shadow_offer_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    for (mode, model) in [
        (FeatureMode::On, Scripted::default().yes(QUESTION, false)),
        (FeatureMode::Off, yes()),
        (FeatureMode::On, Scripted::down()),
        (FeatureMode::On, Scripted::default()),
        (FeatureMode::Shadow, yes()),
    ] {
        install(&handle, FEATURE, mode, model);
        at_turn_start(&ctx, MESSAGE).await;
    }
    let would = |record: &DecisionRecord| record.shadow && record.outcome == "offered to save";
    assert!(
        traced(&handle, would).await,
        "shadow records what it would offer"
    );
    assert!(offered(&events).is_empty());
    handle.close("test").await;
}

#[tokio::test]
async fn unmarked_or_known_rules_are_not_asked_about() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, yes());
    at_turn_start(&ctx, "Use pnpm for this install.").await;
    let mut settings = (*handle.core.settings()).clone();
    settings.instructions = vec![InstructionDoc {
        label: "Project instructions (AGENTS.md)".into(),
        path: "AGENTS.md".into(),
        content: "- Always use pnpm for installs, never npm.\n".into(),
    }];
    *write(&handle.core.settings) = Arc::new(settings);
    at_turn_start(&ctx, MESSAGE).await;
    assert!(offered(&events).is_empty());
    assert!(handle.core.decisions.trace().recent(10).is_empty());
    handle.close("test").await;
}
