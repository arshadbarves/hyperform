//! A language server that never finishes initializing does not stall
//! writes (post-write diagnostics skip it), and closing the session kills
//! it promptly instead of waiting out its initialize timeout.

mod support;

use std::time::{Duration, Instant};

use serde_json::json;
use support::{BASE_SETTINGS, Harness, group_gone, read_pid, results};
use z_engine_protocol::TurnOutcome;
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn writes_do_not_wait_for_a_server_that_is_still_starting() {
    let pids = tempfile::tempdir().unwrap();
    let pid_file = pids.path().join("lsp.pid");
    let script = format!("echo $$ > '{}'; exec sleep 60", pid_file.display());
    let settings = format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n\n[lsp.servers.hung]\ncommand = \"sh\"\nargs = [\"-c\", {script:?}]\nextensions = [\"rs\"]\nroot_markers = [\"Cargo.toml\"]\n"
    );
    let repo = FixtureRepo::git(&[
        ("Cargo.toml", "[package]\nname = \"demo\"\n"),
        ("src/lib.rs", "fn main() {}\n"),
    ]);
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    let lib = h.path("src/lib.rs");
    h.model
        .push(Script::tool("Read", json!({"file_path": lib})));
    for content in ["fn one() {}\n", "fn two() {}\n", "fn three() {}\n"] {
        h.model.push(Script::tool(
            "Write",
            json!({"file_path": lib, "content": content}),
        ));
    }
    h.model.push(Script::text("Done."));
    let started = Instant::now();
    let turn = h.run_turn("edit repeatedly").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(4_500),
        "three writes took {elapsed:?}; post-write diagnostics stalled"
    );
    for request in &h.main_requests()[2..] {
        let written = results(request.messages.last().unwrap());
        assert!(!written[0].1, "{written:?}");
    }

    let pgid = read_pid(&pid_file).await;
    let closing = Instant::now();
    h.engine.close_session(&h.session).await.unwrap();
    assert!(
        closing.elapsed() < Duration::from_secs(10),
        "close waited {:?} for the initializing server",
        closing.elapsed()
    );
    assert!(
        group_gone(pgid).await,
        "the starting server outlived its session"
    );
}
