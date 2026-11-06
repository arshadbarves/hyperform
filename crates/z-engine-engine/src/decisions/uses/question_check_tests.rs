//! The question check only redirects the model once: a question the chat
//! answered gets a pointer to that message, the same question asked again
//! reaches the user, and off, down, unsure, shadow and partly answered
//! calls reach the user at once.

use tokio_util::sync::CancellationToken;
use z_engine_config::FeatureMode;
use z_engine_decisions::DecisionRecord;
use z_engine_protocol::{Message, QuestionOption};

use super::*;
use crate::decisions::seams::review_questions;
use crate::decisions::uses::scripted::{Scripted, install, session, traced};
use crate::session::SessionHandle;

const FEATURE: FeatureId = FeatureId::DecisionsQuestionCheck;

fn question(text: &str, labels: &[&str]) -> Question {
    Question {
        question: text.into(),
        header: "Choice".into(),
        options: labels
            .iter()
            .map(|label| QuestionOption {
                label: (*label).into(),
                description: None,
            })
            .collect(),
        multi_select: false,
    }
}

fn database() -> Question {
    question(
        "Which database should the service use?",
        &["PostgreSQL", "SQLite"],
    )
}

fn chat(handle: &SessionHandle) {
    handle.core.with_state(|state| {
        state
            .transcript
            .push(Message::user_text("Build the orders service."));
        state.transcript.push(Message::assistant_text("Starting."));
        state.transcript.push(Message::user_text(
            "Use PostgreSQL as the database, it is deployed.",
        ));
    });
}

async fn ask(handle: &SessionHandle, questions: &[Question]) -> Option<String> {
    review_questions(&handle.core, &CancellationToken::new(), questions).await
}

fn yes() -> Scripted {
    Scripted::default().yes(QUESTION, true)
}

#[tokio::test]
async fn an_answered_question_points_at_the_message_once() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    chat(&handle);
    install(&handle, FEATURE, FeatureMode::On, yes());
    let note = ask(&handle, &[database()]).await.expect("redirected");
    assert!(note.contains("message 2 said \"Use PostgreSQL"), "{note}");
    assert!(
        note.contains("\"Which database should the service use?\""),
        "{note}"
    );
    let again = question(
        "which  database should the SERVICE use?",
        &["PostgreSQL", "SQLite"],
    );
    assert_eq!(
        ask(&handle, &[again]).await,
        None,
        "asked again: the user is asked"
    );
    let reached = |record: &DecisionRecord| record.outcome == ASKED_AGAIN;
    assert!(traced(&handle, reached).await);
    handle.close("test").await;
}

#[tokio::test]
async fn no_off_down_unsure_and_shadow_reach_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    chat(&handle);
    for (mode, model) in [
        (FeatureMode::On, Scripted::default().yes(QUESTION, false)),
        (FeatureMode::Off, yes()),
        (FeatureMode::On, Scripted::down()),
        (FeatureMode::On, Scripted::default()),
        (FeatureMode::Shadow, yes()),
    ] {
        install(&handle, FEATURE, mode, model);
        assert_eq!(ask(&handle, &[database()]).await, None, "{mode:?}");
    }
    let would = |record: &DecisionRecord| record.shadow && record.outcome.starts_with("pointed");
    assert!(
        traced(&handle, would).await,
        "shadow records what it would do"
    );
    install(&handle, FEATURE, FeatureMode::On, yes());
    assert!(
        ask(&handle, &[database()]).await.is_some(),
        "shadow remembered nothing"
    );
    handle.close("test").await;
}

#[tokio::test]
async fn calls_with_an_unanswered_question_reach_the_user_unasked() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    chat(&handle);
    install(&handle, FEATURE, FeatureMode::On, yes());
    let deploy = question("Where will it be hosted?", &["Fly", "Render"]);
    assert_eq!(ask(&handle, &[database(), deploy]).await, None);
    assert!(
        handle.core.decisions.trace().recent(10).is_empty(),
        "no message shares a word with the second question, so nothing is asked"
    );
    handle.close("test").await;
}
