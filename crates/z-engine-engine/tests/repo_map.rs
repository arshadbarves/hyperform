//! The repository map is the last cached system section and stays
//! byte-identical across the rounds of a session; it can be turned off.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness};
use z_engine_llm::ModelRequest;
use z_engine_testkit::{FixtureRepo, Script};

fn repo() -> FixtureRepo {
    FixtureRepo::git(&[
        (
            "src/lib.rs",
            "pub struct LedgerBook;\n\npub fn balance_ledger(book: &LedgerBook) -> u32 {\n    0\n}\n",
        ),
        (
            "src/main.rs",
            "fn main() {\n    let book = demo::LedgerBook;\n    demo::balance_ledger(&book);\n}\n",
        ),
    ])
}

fn system(request: &ModelRequest) -> Vec<(String, bool)> {
    request
        .system
        .iter()
        .map(|block| (block.text.clone(), block.cache))
        .collect()
}

#[tokio::test]
async fn repo_map_is_present_and_stable() {
    let mut h = Harness::start(repo()).await;
    let lib = h.path("src/lib.rs");
    h.model
        .push(Script::tool("Read", json!({"file_path": lib})));
    h.model.push(Script::text("Read."));
    h.run_turn("read lib").await;
    h.model.push(Script::text("Again."));
    h.run_turn("and again").await;

    let requests = h.main_requests();
    assert_eq!(requests.len(), 3);
    let first = system(&requests[0]);
    let (map, cached) = first.last().unwrap();
    assert!(*cached, "the map carries the system cache breakpoint");
    assert!(map.starts_with("# Repository map"), "{map}");
    assert!(
        map.contains("src/lib.rs:") && map.contains("balance_ledger"),
        "{map}"
    );
    for request in &requests[1..] {
        assert_eq!(system(request), first);
    }
}

#[tokio::test]
async fn repo_map_can_be_disabled() {
    let settings = format!("{BASE_SETTINGS}\n[context]\nrepo_map = false\n");
    let mut h = Harness::builder(repo()).settings(&settings).start().await;
    h.model.push(Script::text("Hi."));
    h.run_turn("hi").await;
    let first = system(&h.main_requests()[0]);
    assert!(
        first
            .iter()
            .all(|(text, _)| !text.contains("Repository map"))
    );
}
