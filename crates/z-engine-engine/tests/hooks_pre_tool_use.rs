//! `PreToolUse` hooks: exit 2 blocks the call with stderr as the reason;
//! `updatedInput` rewrites the call before it runs.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, hook_script, hook_toml, results};
use z_engine_protocol::Event;
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn exit_two_blocks_the_call_with_the_reason() {
    let scripts = tempfile::tempdir().unwrap();
    let block = hook_script(
        scripts.path(),
        "block.sh",
        "cat > /dev/null\necho 'no shell today' >&2\nexit 2\n",
    );
    let settings = format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n{}",
        hook_toml("PreToolUse", Some("Bash"), &block)
    );
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::tool(
        "Bash",
        json!({ "command": "touch blocked.txt" }),
    ));
    h.model.push(Script::text("ok"));
    h.run_turn("make a file").await;

    assert!(!h.repo.exists("blocked.txt"));
    let requests = h.main_requests();
    let answered = results(requests[1].messages.last().unwrap());
    assert!(answered[0].1);
    assert!(
        answered[0].2.contains("no shell today"),
        "{}",
        answered[0].2
    );
    assert_eq!(
        h.events.count(|e| matches!(
            e,
            Event::HookRan { hook_event, blocked: true, .. } if hook_event == "PreToolUse"
        )),
        1
    );
}

#[tokio::test]
async fn updated_input_rewrites_the_call() {
    let scripts = tempfile::tempdir().unwrap();
    let output = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"file_path":"rewritten.txt","content":"from hook"}}}"#;
    let rewrite = hook_script(
        scripts.path(),
        "rewrite.sh",
        &format!("cat > /dev/null\nprintf '%s' '{output}'\n"),
    );
    let settings = format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"acceptEdits\"\n{}",
        hook_toml("PreToolUse", Some("Write"), &rewrite)
    );
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::tool(
        "Write",
        json!({ "file_path": "original.txt", "content": "from model" }),
    ));
    h.model.push(Script::text("ok"));
    h.run_turn("write a file").await;

    assert!(!h.repo.exists("original.txt"));
    assert_eq!(h.repo.read("rewritten.txt"), "from hook");
}
