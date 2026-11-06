//! `UserPromptSubmit` hooks: exit 2 blocks the prompt (nothing is sent),
//! plain stdout of a passing hook becomes context for the model, and the
//! payload's `permission_mode` is `plan` while planning.

mod support;

use support::{BASE_SETTINGS, Harness, hook_script, hook_toml, last_user_text};
use z_engine_protocol::{Command, Event, NoticeLevel};
use z_engine_testkit::{FixtureRepo, Script};

const GUARD: &str = r#"payload=$(cat)
case "$payload" in
  *forbidden*) echo "no forbidden words" >&2; exit 2 ;;
esac
echo "remember the style guide"
"#;

#[tokio::test]
async fn blocked_prompts_are_not_sent_and_context_is_added() {
    let scripts = tempfile::tempdir().unwrap();
    let guard = hook_script(scripts.path(), "guard.sh", GUARD);
    let settings = format!(
        "{BASE_SETTINGS}{}",
        hook_toml("UserPromptSubmit", None, &guard)
    );
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;

    h.submit("this is forbidden");
    h.wait(|e| {
        matches!(
            e,
            Event::Notice { level: NoticeLevel::Warn, text } if text.contains("no forbidden words")
        )
    })
    .await;
    assert_eq!(
        h.events.count(|e| matches!(e, Event::TurnStarted { .. })),
        0
    );
    assert!(h.transcript().is_empty());

    h.model.push(Script::text("hi"));
    h.run_turn("hello").await;
    let requests = h.main_requests();
    assert_eq!(
        requests.len(),
        1,
        "the blocked prompt never reached the model"
    );
    let prompt = last_user_text(&requests[0]);
    assert!(
        prompt.contains("hello") && prompt.contains("remember the style guide"),
        "{prompt}"
    );
}

const MODE_ECHO: &str = r#"payload=$(cat)
case "$payload" in
  *'"permission_mode":"plan"'*) echo "hook saw plan" ;;
  *'"permission_mode":"acceptEdits"'*) echo "hook saw acceptEdits" ;;
esac
"#;

#[tokio::test]
async fn hooks_see_plan_while_planning_and_the_level_otherwise() {
    let scripts = tempfile::tempdir().unwrap();
    let echo = hook_script(scripts.path(), "mode.sh", MODE_ECHO);
    let settings = format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"acceptEdits\"\nplanning = true\n{}",
        hook_toml("UserPromptSubmit", None, &echo)
    );
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::text("planning"));
    h.run_turn("first").await;
    h.send(Command::SetPlanning { planning: false });
    h.wait(|e| matches!(e, Event::PlanningChanged { planning: false }))
        .await;
    h.model.push(Script::text("building"));
    h.run_turn("second").await;
    let requests = h.main_requests();
    assert!(last_user_text(&requests[0]).contains("hook saw plan"));
    assert!(last_user_text(&requests[1]).contains("hook saw acceptEdits"));
}
