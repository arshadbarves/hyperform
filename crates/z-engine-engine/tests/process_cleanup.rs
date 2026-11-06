//! No process outlives its session: background shells, MCP servers and
//! language servers (each recording its own pid) are gone after the
//! session closes, and after `Engine::shutdown`.

mod support;

use std::path::{Path, PathBuf};

use serde_json::json;
use support::{
    BASE_SETTINGS, Harness, exec_recording_pid, fake_lsp_bin, fake_mcp_bin, group_gone,
    group_running, read_pid, skip,
};
use z_engine_protocol::{Event, NoticeLevel, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

struct Running {
    h: Harness,
    groups: Vec<(&'static str, u32)>,
    _pids: tempfile::TempDir,
}

fn toml_args(args: &[String]) -> String {
    let quoted: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    format!("[{}]", quoted.join(", "))
}

fn server_toml(section: &str, pid_file: &Path, bin: &Path, extra: &str) -> String {
    let (command, args) = exec_recording_pid(pid_file, bin);
    format!(
        "\n[{section}]\ncommand = {command:?}\nargs = {}\n{extra}\n",
        toml_args(&args)
    )
}

/// A session with a background shell, an MCP server and a language
/// server running; `None` when the fake servers are unavailable.
async fn session_with_processes() -> Option<Running> {
    let (mcp, lsp) = (fake_mcp_bin()?, fake_lsp_bin()?);
    let pids = tempfile::tempdir().unwrap();
    let pid = |name: &str| -> PathBuf { pids.path().join(name) };
    let settings = format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n{}{}",
        server_toml(
            "mcp.servers.fake",
            &pid("mcp.pid"),
            &mcp,
            "timeout_secs = 10"
        ),
        server_toml(
            "lsp.servers.fake",
            &pid("lsp.pid"),
            &lsp,
            "extensions = [\"rs\"]\nroot_markers = [\"Cargo.toml\"]"
        ),
    );
    let repo = FixtureRepo::git(&[
        ("Cargo.toml", "[package]\nname = \"demo\"\n"),
        ("src/lib.rs", "fn main() {}\n"),
    ]);
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    h.expect(|e| {
        matches!(e, Event::Notice { level: NoticeLevel::Info, text } if text.contains("MCP servers ready: fake"))
    })
    .await;
    let background = format!("echo $$ > '{}'; sleep 30", pid("bg.pid").display());
    h.model.push(Script::tools(&[
        (
            "Bash",
            json!({"command": background, "run_in_background": true}),
        ),
        (
            "LSP",
            json!({"operation": "documentSymbols", "file_path": h.path("src/lib.rs")}),
        ),
    ]));
    h.model.push(Script::text("started"));
    assert_eq!(
        h.run_turn("start things").await.outcome,
        TurnOutcome::Completed
    );
    let mut groups = Vec::new();
    for (name, file) in [
        ("background shell", "bg.pid"),
        ("MCP server", "mcp.pid"),
        ("language server", "lsp.pid"),
    ] {
        let pgid = read_pid(&pid(file)).await;
        assert!(group_running(pgid), "{name} is not running");
        groups.push((name, pgid));
    }
    Some(Running {
        h,
        groups,
        _pids: pids,
    })
}

async fn assert_all_gone(groups: &[(&str, u32)]) {
    for (name, pgid) in groups {
        assert!(group_gone(*pgid).await, "the {name} outlived its session");
    }
}

#[tokio::test]
async fn closing_a_session_stops_its_processes() {
    let Some(running) = session_with_processes().await else {
        return skip("the fake MCP and LSP servers");
    };
    running
        .h
        .engine
        .close_session(&running.h.session)
        .await
        .unwrap();
    assert_all_gone(&running.groups).await;
}

#[tokio::test]
async fn engine_shutdown_stops_every_process() {
    let Some(running) = session_with_processes().await else {
        return skip("the fake MCP and LSP servers");
    };
    running.h.engine.shutdown().await;
    assert_all_gone(&running.groups).await;
}
