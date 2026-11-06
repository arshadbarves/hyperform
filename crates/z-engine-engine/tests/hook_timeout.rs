//! `PreToolUse` and `PostToolUse` hooks that hang past their timeout are
//! killed (their whole process tree), reported with a warning, and neither
//! block nor fail the call or the turn.

mod support;

use std::time::{Duration, Instant};

use serde_json::json;
use support::{BASE_SETTINGS, Harness, group_gone, hook_script, read_pid, results};
use z_engine_protocol::{Event, NoticeLevel, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

fn hanging_hook(dir: &std::path::Path, event: &str) -> (String, std::path::PathBuf) {
    let pid_file = dir.join(format!("{event}.pid"));
    let body = format!(
        "echo $$ > '{}'\ncat > /dev/null\nsleep 30\n",
        pid_file.display()
    );
    let command = hook_script(dir, &format!("{event}.sh"), &body);
    let toml = format!(
        "\n[[hooks.{event}]]\nmatcher = \"Bash\"\ncommand = \"{command}\"\ntimeout_secs = 1\n"
    );
    (toml, pid_file)
}

#[tokio::test]
async fn hung_tool_hooks_time_out_without_blocking_the_turn() {
    let scripts = tempfile::tempdir().unwrap();
    let (pre, pre_pid) = hanging_hook(scripts.path(), "PreToolUse");
    let (post, post_pid) = hanging_hook(scripts.path(), "PostToolUse");
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n{pre}{post}");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::tool(
        "Bash",
        json!({ "command": "echo ran > ran.txt" }),
    ));
    h.model.push(Script::text("ok"));
    let started = Instant::now();
    let turn = h.run_turn("run it").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "hooks held the turn for {:?}",
        started.elapsed()
    );
    assert!(
        h.repo.exists("ran.txt"),
        "a timed-out PreToolUse hook does not block"
    );
    let answered = results(h.main_requests()[1].messages.last().unwrap());
    assert!(!answered[0].1, "{answered:?}");

    let timeouts = h.events.count(|e| {
        matches!(e, Event::Notice { level: NoticeLevel::Warn, text } if text.contains("timed out"))
    });
    assert_eq!(timeouts, 2, "{:#?}", h.events.seen());
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::HookRan { blocked: true, .. })),
        0
    );
    for pid_file in [pre_pid, post_pid] {
        let pgid = read_pid(&pid_file).await;
        assert!(
            group_gone(pgid).await,
            "hook {} still runs",
            pid_file.display()
        );
    }
}
