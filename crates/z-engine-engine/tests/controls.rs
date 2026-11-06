//! Model and effort switches apply to the next request, persist across a
//! reopen, and the prompt inspector shows the last request.

mod support;

use support::Harness;
use z_engine_protocol::{Command, Effort, Event};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn model_and_effort_changes_apply_and_persist() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.send(Command::SetModel {
        model: "other-model".into(),
    });
    h.wait(|e| matches!(e, Event::ModelChanged { model } if model == "other-model"))
        .await;
    h.send(Command::SetEffort {
        effort: Some(Effort::High),
    });
    h.wait(|e| {
        matches!(
            e,
            Event::EffortChanged {
                effort: Some(Effort::High)
            }
        )
    })
    .await;

    h.model.push(Script::text("switched"));
    h.run_turn("hello").await;
    let request = h.main_requests().pop().unwrap();
    assert_eq!(request.model, "other-model");
    assert_eq!(request.thinking.map(|t| t.effort), Some(Effort::High));
    assert!(request.cache_tools && !request.cache_breakpoints.is_empty());
    assert!(request.system.iter().any(|block| block.cache));

    let inspected = h.engine.last_request(&h.session).unwrap();
    assert_eq!(inspected["model"], "other-model");
    assert_eq!(inspected["thinking"]["effort"], "high");
    assert!(
        inspected["messages"]
            .as_array()
            .is_some_and(|m| !m.is_empty())
    );

    h.reopen().await;
    match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => {
            assert_eq!(snapshot.info.model, "other-model");
            assert_eq!(snapshot.info.effort, Some(Effort::High));
        }
        other => panic!("{other:?}"),
    }
}
