//! `/model`, `/mode` and `/effort` change the session like the matching
//! GUI controls and confirm with command output; `/mode plan` toggles plan
//! mode like Shift+Tab.

mod support;

use hyperform_protocol::{Effort, Event, PermissionMode};
use hyperform_testkit::{FixtureRepo, Script};
use support::Harness;

#[tokio::test]
async fn model_mode_and_effort_commands() {
    let mut h = Harness::start(FixtureRepo::empty()).await;

    h.command("model", "other-model");
    h.expect(|e| matches!(e, Event::ModelChanged { model } if model == "other-model"))
        .await;
    assert!(h.command_output("model").await.contains("`other-model`"));

    h.command("mode", "plan");
    h.expect(|e| matches!(e, Event::PlanningChanged { planning: true }))
        .await;
    assert!(h.command_output("mode").await.contains("plan mode: **on**"));
    h.command("mode", "acceptEdits");
    h.expect(|e| {
        matches!(
            e,
            Event::ModeChanged {
                mode: PermissionMode::AcceptEdits
            }
        )
    })
    .await;
    let report = h.command_output("mode").await;
    assert!(
        report.contains("**acceptEdits**") && report.contains("plan mode: **on**"),
        "a level change keeps plan mode: {report}"
    );
    h.command("mode", "plan");
    h.expect(|e| matches!(e, Event::PlanningChanged { planning: false }))
        .await;

    h.command("effort", "high");
    h.expect(|e| {
        matches!(
            e,
            Event::EffortChanged {
                effort: Some(Effort::High)
            }
        )
    })
    .await;
    assert!(h.command_output("effort").await.contains("high"));
    h.command("effort", "default");
    h.expect(|e| matches!(e, Event::EffortChanged { effort: None }))
        .await;

    h.command("mode", "sideways");
    h.notice("Usage: /mode").await;

    h.model.push(Script::text("ok"));
    h.run_turn("hi").await;
    assert_eq!(h.main_requests().pop().unwrap().model, "other-model");
}
