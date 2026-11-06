//! `AskUserQuestion`: an answer reaches the model; a dismissal resolves
//! the card and tells the model the user declined.

mod support;

use serde_json::{Value, json};
use support::{Harness, results};
use z_engine_protocol::{Command, Event, QuestionAnswer, RequestId};
use z_engine_testkit::{FixtureRepo, Script};

fn question() -> Value {
    json!({ "questions": [{
        "question": "Which color?",
        "header": "Color",
        "options": [
            { "label": "Red", "description": "warm" },
            { "label": "Blue", "description": "cool" }
        ],
        "multiSelect": false
    }]})
}

async fn asked(h: &mut Harness) -> RequestId {
    match h.wait(|e| matches!(e, Event::QuestionAsked { .. })).await {
        Event::QuestionAsked {
            request_id,
            questions,
            ..
        } => {
            assert_eq!(questions[0].question, "Which color?");
            request_id
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn answers_reach_the_model() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::tool("AskUserQuestion", question()));
    h.model.push(Script::text("Blue it is."));
    h.submit("pick a color with me");
    let request_id = asked(&mut h).await;
    h.send(Command::AnswerQuestion {
        request_id,
        answers: Some(vec![QuestionAnswer {
            question: "Which color?".into(),
            answers: vec!["Blue".into()],
        }]),
    });
    h.wait(|e| matches!(e, Event::QuestionResolved { answered: true, .. }))
        .await;
    h.turn_finished().await;
    let requests = h.main_requests();
    let answered = results(requests[1].messages.last().unwrap());
    assert!(!answered[0].1);
    assert!(answered[0].2.contains("Blue"), "{}", answered[0].2);
}

#[tokio::test]
async fn dismissed_questions_resolve_unanswered() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::tool("AskUserQuestion", question()));
    h.model.push(Script::text("Okay, I will choose."));
    h.submit("pick a color with me");
    let request_id = asked(&mut h).await;
    h.send(Command::AnswerQuestion {
        request_id,
        answers: None,
    });
    h.wait(|e| {
        matches!(
            e,
            Event::QuestionResolved {
                answered: false,
                ..
            }
        )
    })
    .await;
    h.turn_finished().await;
    let requests = h.main_requests();
    let answered = results(requests[1].messages.last().unwrap());
    assert_eq!(answered.len(), 1);
    assert!(!answered[0].2.contains("Blue"));
}
