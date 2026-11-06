use tokio_util::sync::CancellationToken;
use z_engine_llm::{ModelClient, ModelRequest, SystemBlock, collect};
use z_engine_protocol::Message;
use z_engine_testkit::{FixtureRepo, Script, ScriptedModel};

#[tokio::test]
async fn queued_scripts_and_routes_answer_in_order() {
    let model = ScriptedModel::new();
    model.route_system("title writer", Script::text("A Title"));
    model.push(Script::tool(
        "Read",
        serde_json::json!({"file_path": "a.rs"}),
    ));
    model.push(Script::text("finished"));

    let main = ModelRequest::new("m", vec![Message::user_text("hi")]);
    let first = collect(model.stream(main.clone(), CancellationToken::new()))
        .await
        .unwrap();
    assert!(first.has_tool_use());

    let side = ModelRequest::new("m", vec![Message::user_text("x")])
        .with_system(vec![SystemBlock::new("You are a title writer.")]);
    let title = collect(model.stream(side, CancellationToken::new()))
        .await
        .unwrap();
    assert_eq!(title.text(), "A Title");

    let second = collect(model.stream(main, CancellationToken::new()))
        .await
        .unwrap();
    assert_eq!(second.text(), "finished");
    assert_eq!(model.requests().len(), 3);
    assert_eq!(model.remaining(), 0);
}

#[tokio::test]
async fn stall_ends_on_cancel() {
    let model = ScriptedModel::new();
    model.push(Script::Stall(vec![]));
    let cancel = CancellationToken::new();
    let mut stream = model.stream(ModelRequest::new("m", vec![]), cancel.clone());
    cancel.cancel();
    assert!(stream.recv().await.is_none());
}

#[test]
fn fixture_repo_commits_files() {
    let repo = FixtureRepo::git(&[("src/lib.rs", "pub fn a() {}\n")]);
    assert!(repo.exists("src/lib.rs"));
    let log = repo.run_git(&["log", "--oneline"]);
    assert!(log.contains("initial"));
}
