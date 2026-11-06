//! `decisions_question_check` end to end against a local decision model
//! that says yes to everything: a question the chat already answered comes
//! back to the model as a pointer to that message, and the same question
//! asked again reaches the user.

mod laya;
mod support;

use serde_json::{Value, json};
use support::{BASE_SETTINGS, Harness, results};
use z_engine_protocol::{Command, Event, QuestionAnswer};
use z_engine_testkit::{FixtureRepo, Script};

fn yes(_name: &str, _state: &Value) -> &'static str {
    laya::YES
}

fn question() -> Value {
    json!({ "questions": [{
        "question": "Which database should the service use?",
        "header": "Database",
        "options": [
            { "label": "PostgreSQL", "description": "relational" },
            { "label": "SQLite", "description": "embedded" }
        ],
        "multiSelect": false
    }]})
}

#[tokio::test]
async fn an_answered_question_points_back_once_then_reaches_the_user() {
    let endpoint = laya::laya(yes).await;
    let settings = laya::settings(BASE_SETTINGS, &endpoint, &["decisions_question_check"]);
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::text("Noted."));
    h.run_turn("Use PostgreSQL for the database.").await;

    h.model.push(Script::tool("AskUserQuestion", question()));
    h.model.push(Script::tool("AskUserQuestion", question()));
    h.model.push(Script::text("Setting it up with PostgreSQL."));
    h.submit("Set up the orders service now");
    let request_id = match h.wait(|e| matches!(e, Event::QuestionAsked { .. })).await {
        Event::QuestionAsked { request_id, .. } => request_id,
        other => panic!("{other:?}"),
    };
    h.send(Command::AnswerQuestion {
        request_id,
        answers: Some(vec![QuestionAnswer {
            question: "Which database should the service use?".into(),
            answers: vec!["PostgreSQL".into()],
        }]),
    });
    h.turn_finished().await;

    let requests = h.main_requests();
    assert_eq!(requests.len(), 4);
    let redirected = results(requests[2].messages.last().unwrap());
    assert!(!redirected[0].1);
    assert!(
        redirected[0].2.contains("message 1 said \"Use PostgreSQL"),
        "{}",
        redirected[0].2
    );
    let answered = results(requests[3].messages.last().unwrap());
    assert!(
        answered[0].2.contains("The user answered"),
        "{}",
        answered[0].2
    );
    h.events.drain();
    let asked = h.events.seen().iter();
    let asked = asked.filter(|e| matches!(e, Event::QuestionAsked { .. }));
    assert_eq!(asked.count(), 1, "the user was asked exactly once");
}
