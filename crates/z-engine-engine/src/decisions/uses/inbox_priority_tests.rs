//! Inbox priority: confident scores reach the GUI and hold back
//! `Notification` hooks only when nothing is high; off, down, unsure and
//! shadow score nothing, so every hook fires as before.

use tokio_util::sync::CancellationToken;
use z_engine_config::FeatureMode;
use z_engine_decisions::DecisionRecord;
use z_engine_protocol::decisions::UrgencyInfo;
use z_engine_protocol::{Event, NoticeLevel};

use super::*;
use crate::decisions::seams::{AttentionKind, score_attention_with, worth_notifying};
use crate::decisions::uses::scripted::{Events, Scripted, install, session, traced};
use crate::session::SessionHandle;

const FEATURE: FeatureId = FeatureId::DecisionsInboxPriority;

fn items() -> Vec<AttentionItem> {
    vec![
        AttentionItem::request(AttentionKind::Question, "req_1", "Which branch?".into()),
        AttentionItem::notice(NoticeLevel::Warn, "a hook timed out"),
    ]
}

fn scored(events: &Events) -> Vec<UrgencyInfo> {
    let events = events.lock().unwrap();
    let scores = events.iter().filter_map(|event| match event {
        Event::UrgencyScored { urgency } => Some(urgency.clone()),
        _ => None,
    });
    scores.collect()
}

async fn score(handle: &SessionHandle) -> Vec<Option<Urgency>> {
    let cancel = CancellationToken::new();
    score_attention_with(&[&INBOX_PRIORITY], &handle.core, items(), &cancel).await
}

#[tokio::test]
async fn confident_scores_reach_the_gui_and_gate_notifications() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(
        &handle,
        FEATURE,
        FeatureMode::On,
        Scripted::default().choice(QUESTION, "low"),
    );
    let low = score(&handle).await;
    assert_eq!(low, [Some(Urgency::Low), Some(Urgency::Low)]);
    assert!(
        !worth_notifying(&low),
        "nothing urgent: hooks are held back"
    );
    let keys: Vec<String> = scored(&events).into_iter().map(|info| info.key).collect();
    assert_eq!(keys, ["req_1", "notice:a hook timed out"]);
    install(
        &handle,
        FEATURE,
        FeatureMode::On,
        Scripted::default().choice(QUESTION, "high"),
    );
    assert!(worth_notifying(&score(&handle).await));
    let records = handle.core.decisions.trace().recent(10);
    assert!(records.iter().any(|r| r.outcome == "rated high"));
    handle.close("test").await;
}

#[tokio::test]
async fn off_down_unsure_and_shadow_score_nothing_and_notify_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let low = || Scripted::default().choice(QUESTION, "low");
    install(&handle, FEATURE, FeatureMode::Off, low());
    assert_eq!(score(&handle).await, [None, None]);
    for model in [Scripted::down(), Scripted::default()] {
        install(&handle, FEATURE, FeatureMode::On, model);
        assert!(worth_notifying(&score(&handle).await));
    }
    install(&handle, FEATURE, FeatureMode::Shadow, low());
    assert!(worth_notifying(&score(&handle).await));
    let would = |r: &DecisionRecord| r.shadow && r.outcome == "would rate low";
    assert!(
        traced(&handle, would).await,
        "shadow records what it would do"
    );
    assert!(scored(&events).is_empty());
    handle.close("test").await;
}

#[tokio::test]
async fn warning_and_error_notices_are_scored_in_the_background() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(
        &handle,
        FEATURE,
        FeatureMode::On,
        Scripted::default().choice(QUESTION, "high"),
    );
    handle.core.events.notice(NoticeLevel::Info, "routine");
    handle
        .core
        .events
        .notice(NoticeLevel::Warn, "disk almost full");
    let wanted = "notice:disk almost full";
    let mut keys = Vec::new();
    for _ in 0..200 {
        keys = scored(&events).into_iter().map(|info| info.key).collect();
        if keys.iter().any(|key| key == wanted) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(keys, [wanted], "info notices are not scored");
    handle.close("test").await;
}

#[test]
fn every_urgency_key_maps() {
    assert_eq!(urgency_of("low"), Some(Urgency::Low));
    assert_eq!(urgency_of("normal"), Some(Urgency::Normal));
    assert_eq!(urgency_of("high"), Some(Urgency::High));
    assert_eq!(urgency_of("urgent"), None);
}
