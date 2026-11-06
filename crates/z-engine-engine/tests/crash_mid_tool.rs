//! The app dies while a long Bash call runs (the runtime is torn down under
//! the engine, without close or shutdown). The shell's whole process tree
//! goes with it, and a restarted engine reopens the session with the turn
//! `Interrupted`, the call answered as interrupted, and a working next turn.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, assert_valid_request, group_gone, read_pid, results};
use z_engine_protocol::{Event, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn a_crash_mid_tool_kills_the_tree_and_reopens_interrupted() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n");
    let crashed = runtime();
    let mut h = crashed.block_on(
        Harness::builder(FixtureRepo::empty())
            .settings(&settings)
            .start(),
    );
    let pid_file = h.repo.path().join("shell.pid");
    h.model.push(Script::tool(
        "Bash",
        json!({
            "command": format!("echo $$ > '{}'; sleep 30; echo done", pid_file.display()),
            "timeout": 60_000
        }),
    ));
    let pgid = crashed.block_on(async {
        h.submit("run the long command");
        h.wait(|e| matches!(e, Event::ToolStarted { .. })).await;
        read_pid(&pid_file).await
    });
    let remains = h.crash(crashed);

    let restarted = runtime();
    restarted.block_on(async {
        assert!(group_gone(pgid).await, "the Bash tree outlived the crash");
        let mut h = remains.restart().await;
        let snapshot = match h.expect(|e| matches!(e, Event::Snapshot { .. })).await {
            Event::Snapshot { snapshot } => snapshot,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            snapshot.turns.last().map(|turn| &turn.outcome),
            Some(&TurnOutcome::Interrupted)
        );
        let answered = results(snapshot.messages.last().unwrap());
        assert_eq!(answered.len(), 1, "{answered:?}");
        assert!(answered[0].1 && answered[0].2.contains("interrupted"));

        h.model.push(Script::text("picking up"));
        let turn = h.run_turn("what happened?").await;
        assert_eq!(turn.outcome, TurnOutcome::Completed);
        assert_valid_request(&h.main_requests().pop().unwrap());
        h.engine.shutdown().await;
    });
}
