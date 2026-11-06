//! A `Stop` hook that blocks forces one more round with its reason; with
//! `stop_hook_active` set on the next stop it lets the turn end.

mod support;

use support::{BASE_SETTINGS, Harness, hook_script, hook_toml, last_user_text};
use z_engine_protocol::{Event, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

const STOP_ONCE: &str = r#"payload=$(cat)
case "$payload" in
  *'"stop_hook_active":true'*) exit 0 ;;
esac
echo "run the tests first" >&2
exit 2
"#;

#[tokio::test]
async fn blocking_stop_hook_continues_the_turn_once() {
    let scripts = tempfile::tempdir().unwrap();
    let stop = hook_script(scripts.path(), "stop.sh", STOP_ONCE);
    let settings = format!("{BASE_SETTINGS}{}", hook_toml("Stop", None, &stop));
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::text("first answer"));
    h.model.push(Script::text("tests pass now"));
    let turn = h.run_turn("finish the work").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);

    let requests = h.main_requests();
    assert_eq!(requests.len(), 2);
    let feedback = last_user_text(&requests[1]);
    assert!(feedback.contains("run the tests first"), "{feedback}");
    let stops = |blocked: bool| {
        h.events.count(|e| matches!(
            e,
            Event::HookRan { hook_event, blocked: b, .. } if hook_event == "Stop" && *b == blocked
        ))
    };
    assert_eq!((stops(true), stops(false)), (1, 1));
}
