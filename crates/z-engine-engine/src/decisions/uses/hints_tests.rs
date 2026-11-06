//! Hints only add a reminder naming installed skills or agents; off, down,
//! unsure and shadow add nothing, candidates sharing no word with the
//! request are never asked about, and each one is hinted once.

use std::path::Path;
use std::sync::Arc;

use z_engine_config::{ExtensionScope, ExtensionSource, FeatureMode, SkillDef, parse_agent};
use z_engine_decisions::DecisionRecord;

use super::*;
use crate::decisions::seams::at_turn_start;
use crate::decisions::uses::scripted::{Scripted, install, main_run, session, traced};
use crate::session::SessionHandle;
use crate::sync::write;

const FEATURE: FeatureId = FeatureId::DecisionsHints;
const PDF_TASK: &str = "Extract the tables from the quarterly report PDF into a CSV file";

fn source() -> ExtensionSource {
    ExtensionSource::new(ExtensionScope::User, Path::new("/x"))
}

fn installed(handle: &SessionHandle) {
    let mut settings = (*handle.core.settings()).clone();
    let skill = |name: &str, description: &str| SkillDef {
        name: name.into(),
        description: description.into(),
        allowed_tools: Vec::new(),
        dir: "/x".into(),
        file: "/x/SKILL.md".into(),
        source: source(),
    };
    settings.extensions.skills = vec![
        skill("pdf", "Read PDF files and extract their tables and text."),
        skill("slides", "Build presentation decks."),
    ];
    let agent = "---\nname: data-wrangler\ndescription: Converts tables between CSV and JSON.\n---\nYou convert data.";
    settings.extensions.agents = vec![parse_agent(agent, source()).unwrap()];
    *write(&handle.core.settings) = Arc::new(settings);
}

#[tokio::test]
async fn confident_fits_become_one_reminder_naming_them_once() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    installed(&handle);
    let ctx = main_run(&handle);
    install(
        &handle,
        FEATURE,
        FeatureMode::On,
        Scripted::default().yes(QUESTION, true),
    );
    at_turn_start(&ctx, PDF_TASK).await;
    let reminders = handle.core.reminders.take(&ctx.spec.agent_id);
    assert_eq!(reminders.len(), 1);
    let note = &reminders[0];
    assert!(note.contains("- skill `pdf`:"), "{note}");
    assert!(note.contains("- agent `data-wrangler`:"), "{note}");
    assert!(
        !note.contains("slides"),
        "no shared word, never asked: {note}"
    );
    let records = handle.core.decisions.trace().recent(10);
    assert_eq!(records.len(), 2, "one yes/no per shortlisted candidate");
    at_turn_start(&ctx, PDF_TASK).await;
    assert!(
        handle.core.reminders.take(&ctx.spec.agent_id).is_empty(),
        "hinted once"
    );
    handle.close("test").await;
}

#[tokio::test]
async fn no_off_down_unsure_and_shadow_add_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    installed(&handle);
    let ctx = main_run(&handle);
    for (mode, model) in [
        (FeatureMode::On, Scripted::default().yes(QUESTION, false)),
        (FeatureMode::Off, Scripted::default().yes(QUESTION, true)),
        (FeatureMode::On, Scripted::down()),
        (FeatureMode::On, Scripted::default()),
    ] {
        install(&handle, FEATURE, mode, model);
        at_turn_start(&ctx, PDF_TASK).await;
    }
    assert!(handle.core.reminders.take(&ctx.spec.agent_id).is_empty());
    install(
        &handle,
        FEATURE,
        FeatureMode::Shadow,
        Scripted::default().yes(QUESTION, true),
    );
    at_turn_start(&ctx, PDF_TASK).await;
    let would = |record: &DecisionRecord| record.shadow && record.outcome == "hinted";
    assert!(
        traced(&handle, would).await,
        "shadow records what it would hint"
    );
    assert!(handle.core.reminders.take(&ctx.spec.agent_id).is_empty());
    handle.close("test").await;
}

#[tokio::test]
async fn short_messages_and_empty_catalogs_are_not_asked_about() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let ctx = main_run(&handle);
    install(
        &handle,
        FEATURE,
        FeatureMode::On,
        Scripted::default().yes(QUESTION, true),
    );
    at_turn_start(&ctx, PDF_TASK).await;
    installed(&handle);
    at_turn_start(&ctx, "pdf please").await;
    assert!(handle.core.reminders.take(&ctx.spec.agent_id).is_empty());
    assert!(handle.core.decisions.trace().recent(10).is_empty());
    handle.close("test").await;
}
