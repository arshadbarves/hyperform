//! Pet mood: a confident tone reaches the GUI after the turn ended, never
//! for stopped turns; off, down, unsure and shadow send nothing.

use z_engine_config::FeatureMode;
use z_engine_decisions::DecisionRecord;
use z_engine_protocol::{Event, MessageId, TurnId, Usage};

use super::*;
use crate::decisions::seams::turn_end_work;
use crate::decisions::uses::scripted::{Events, Scripted, install, session, traced};

const FEATURE: FeatureId = FeatureId::DecisionsPetMood;

fn turn(outcome: TurnOutcome) -> TurnRecord {
    TurnRecord {
        turn_id: TurnId::from("turn_1"),
        message_id: MessageId::from("m1"),
        outcome,
        verification: VerificationOutcome::Verified {
            checks: vec!["c1".into()],
        },
        usage: Usage::default(),
        cost_usd: 0.0,
        started_at: 0,
        finished_at: 1,
    }
}

fn tones(events: &Events) -> Vec<TurnTone> {
    let events = events.lock().unwrap();
    let judged = events.iter().filter_map(|event| match event {
        Event::TurnToneJudged { tone, .. } => Some(*tone),
        _ => None,
    });
    judged.collect()
}

async fn end(handle: &crate::session::SessionHandle, outcome: TurnOutcome) {
    if let Some(work) = turn_end_work(&[&PET_MOOD], &handle.core, &turn(outcome)) {
        work.await;
    }
}

#[test]
fn every_tone_key_maps_and_stopped_turns_are_not_judged() {
    for key in ["smooth", "struggling", "blocked", "done_well"] {
        assert!(tone_of(key).is_some(), "{key}");
    }
    assert_eq!(tone_of("great"), None);
    assert_eq!(outcome_label(&TurnOutcome::Cancelled), None);
    assert_eq!(outcome_label(&TurnOutcome::Interrupted), None);
}

#[tokio::test]
async fn a_confident_tone_reaches_the_gui_when_on() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let model = Scripted::default().choice(QUESTION, "done_well");
    install(&handle, FEATURE, FeatureMode::On, model.clone());
    end(&handle, TurnOutcome::Completed).await;
    assert_eq!(tones(&events), [TurnTone::DoneWell]);
    let records = handle.core.decisions.trace().recent(10);
    assert!(records.iter().any(|r| r.outcome == "showed done_well"));
    end(&handle, TurnOutcome::Cancelled).await;
    assert_eq!(tones(&events).len(), 1, "stopped turns are not judged");
    handle.close("test").await;
}

#[tokio::test]
async fn off_down_unsure_and_shadow_send_no_tone() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let smooth = || Scripted::default().choice(QUESTION, "smooth");
    install(&handle, FEATURE, FeatureMode::Off, smooth());
    assert!(turn_end_work(&[&PET_MOOD], &handle.core, &turn(TurnOutcome::Completed)).is_none());
    for model in [Scripted::down(), Scripted::default()] {
        install(&handle, FEATURE, FeatureMode::On, model);
        end(&handle, TurnOutcome::Completed).await;
    }
    install(&handle, FEATURE, FeatureMode::Shadow, smooth());
    end(&handle, TurnOutcome::Completed).await;
    let would = |r: &DecisionRecord| r.shadow && r.outcome == "would show smooth";
    assert!(
        traced(&handle, would).await,
        "shadow records what it would do"
    );
    assert!(tones(&events).is_empty());
    handle.close("test").await;
}
