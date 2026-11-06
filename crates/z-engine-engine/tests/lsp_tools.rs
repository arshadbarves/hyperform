//! The `LSP` tool answers through the configured language server, and a
//! written file the server has analyzed gets its errors appended to the
//! write's result; a file it never saw is not waited on (but is analyzed
//! for its next write).

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, fake_lsp_bin, results, skip, tool_names};
use z_engine_protocol::TurnOutcome;
use z_engine_testkit::{FixtureRepo, Script};

const LIB: &str = "struct Greeter {\n    name: String,\n}\n\nfn greet(g: &Greeter) -> String {\n    g.name.clone()\n}\n";

#[tokio::test]
async fn lsp_queries_and_post_edit_errors() {
    let Some(lsp) = fake_lsp_bin() else {
        return skip("zengine-fake-lsp");
    };
    let repo = FixtureRepo::git(&[
        ("Cargo.toml", "[package]\nname = \"demo\"\n"),
        ("src/lib.rs", LIB),
    ]);
    let settings = format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n\n[lsp.servers.fake]\ncommand = {:?}\nextensions = [\"rs\"]\nroot_markers = [\"Cargo.toml\"]\n",
        lsp.display().to_string()
    );
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    let lib = h.path("src/lib.rs");
    h.model.push(Script::tools(&[
        (
            "LSP",
            json!({"operation": "documentSymbols", "file_path": lib}),
        ),
        ("Read", json!({"file_path": lib})),
    ]));
    h.model.push(Script::tools(&[
        (
            "Write",
            json!({"file_path": lib, "content": "fn broken() {} // ERROR here\n"}),
        ),
        (
            "Write",
            json!({"file_path": h.path("src/clean.rs"), "content": "fn clean() {}\n"}),
        ),
    ]));
    h.model.push(Script::tool(
        "Write",
        json!({"file_path": h.path("src/clean.rs"), "content": "fn now() {} // ERROR too\n"}),
    ));
    h.model.push(Script::text("Done."));
    let turn = h.run_turn("look and write").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);

    let requests = h.main_requests();
    assert!(tool_names(&requests[0]).iter().any(|tool| tool == "LSP"));
    let symbols = results(requests[1].messages.last().unwrap());
    assert!(!symbols[0].1, "{symbols:?}");
    assert!(
        symbols[0].2.contains("Greeter") && symbols[0].2.contains("greet"),
        "{symbols:?}"
    );

    let written = results(requests[2].messages.last().unwrap());
    let broken = &written[0].2;
    assert!(broken.contains("<system-reminder>"), "{broken}");
    assert!(broken.contains("errors in src/lib.rs"), "{broken}");
    assert!(broken.contains("found an ERROR marker"), "{broken}");
    assert!(
        !written[1].2.contains("<system-reminder>"),
        "a file the server never analyzed is not waited on: {:?}",
        written[1]
    );
    let rewritten = &results(requests[3].messages.last().unwrap())[0].2;
    assert!(
        rewritten.contains("errors in src/clean.rs"),
        "once analyzed, its next write is checked: {rewritten}"
    );
}
