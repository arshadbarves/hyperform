//! Worktree isolation: the agent edits only its worktree, finishes with
//! pending committed changes and a diffstat, `ApplyAgentChanges` merges
//! them cleanly, a conflicting tree leaves them `Conflicted` without
//! touching the tree, and discarding removes the worktree and branch.
//! Outside git the agent falls back to the shared tree with a notice.

mod support;

use std::path::Path;

use serde_json::{Value, json};
use support::{BASE_SETTINGS, Harness, agent_call, results, route_task, task_requests};
use z_engine_protocol::{
    AgentId, AgentInfo, Command, Event, Isolation, VerificationOutcome, WorktreeInfo, WorktreeState,
};
use z_engine_testkit::{FixtureRepo, Script};

fn bypass() -> String {
    format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n")
}

fn isolated(marker_prompt: &str) -> Value {
    let mut call = agent_call("general", "isolated work", marker_prompt);
    call["isolation"] = json!("worktree");
    call
}

fn worktree_of(info: &AgentInfo) -> WorktreeInfo {
    info.worktree.clone().expect("a worktree agent")
}

async fn worktree_state(h: &mut Harness, agent: &AgentId, state: WorktreeState) -> WorktreeInfo {
    let agent = agent.clone();
    let event = h
        .expect(move |e| {
            matches!(e, Event::AgentUpdated { info } if info.agent_id == agent
                && info.worktree.as_ref().is_some_and(|w| w.state == state))
        })
        .await;
    match event {
        Event::AgentUpdated { info } => worktree_of(&info),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn worktree_changes_apply_cleanly() {
    let repo = FixtureRepo::git(&[("base.txt", "base\n")]);
    let mut h = Harness::builder(repo).settings(&bypass()).start().await;
    let escape = h.path("escape.txt");
    route_task(
        &h.model,
        "WT-TASK",
        vec![
            Script::tools(&[
                (
                    "Write",
                    json!({ "file_path": "feature.txt", "content": "feature\n" }),
                ),
                ("Write", json!({ "file_path": escape, "content": "no" })),
            ]),
            Script::text("added feature.txt"),
        ],
    );
    h.model
        .push(Script::tool("Agent", isolated("WT-TASK add a feature")));
    h.model.push(Script::text("reviewing"));
    h.run_turn("implement in isolation").await;

    let agent = h.agent_started().await.agent_id;
    let pending = worktree_of(&h.agent_finished(&agent).await);
    assert_eq!(pending.state, WorktreeState::Pending);
    assert_eq!(pending.files_changed, 1);
    assert!(
        pending.diffstat.contains("feature.txt"),
        "{}",
        pending.diffstat
    );
    assert_eq!(pending.branch, format!("zengine/{agent}"));
    assert!(Path::new(&pending.path).join("feature.txt").is_file());
    assert!(!h.repo.exists("feature.txt"));
    assert!(!h.repo.exists("escape.txt"));
    let child = task_requests(&h.model, "WT-TASK");
    assert!(child[0].system_text().contains("Isolated git worktree"));
    let writes = results(child[1].messages.last().unwrap());
    assert!(!writes[0].1, "{}", writes[0].2);
    assert!(
        writes[1].1 && writes[1].2.contains("isolated worktree"),
        "{}",
        writes[1].2
    );
    let report = results(h.main_requests()[1].messages.last().unwrap());
    assert!(report[0].2.contains("ApplyAgentChanges"), "{}", report[0].2);

    h.model.push(Script::tool(
        "ApplyAgentChanges",
        json!({ "agent_id": agent }),
    ));
    h.model.push(Script::text("applied"));
    let turn = h.run_turn("apply it").await;
    let applied = results(h.main_requests()[3].messages.last().unwrap());
    assert!(!applied[0].1, "{}", applied[0].2);
    assert_eq!(h.repo.read("feature.txt"), "feature\n");
    assert_ne!(turn.verification, VerificationOutcome::NotApplicable);
    worktree_state(&mut h, &agent, WorktreeState::Applied).await;
    assert!(!Path::new(&pending.path).exists());
    assert!(
        h.repo
            .run_git(&["branch", "--list", "zengine/*"])
            .trim()
            .is_empty()
    );
}

#[tokio::test]
async fn outside_git_the_agent_shares_the_tree() {
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&bypass())
        .start()
        .await;
    route_task(
        &h.model,
        "PLAIN-TASK",
        vec![
            Script::tool(
                "Write",
                json!({ "file_path": "plain.txt", "content": "shared\n" }),
            ),
            Script::text("wrote plain.txt"),
        ],
    );
    h.model
        .push(Script::tool("Agent", isolated("PLAIN-TASK write it")));
    h.model.push(Script::text("ok"));
    h.run_turn("isolate if you can").await;

    let started = h.agent_started().await;
    assert_eq!(started.isolation, Isolation::Shared);
    assert!(started.worktree.is_none());
    h.expect(|e| matches!(e, Event::Notice { text, .. } if text.contains("not a git repository")))
        .await;
    assert_eq!(h.repo.read("plain.txt"), "shared\n");
    let report = results(h.main_requests()[1].messages.last().unwrap());
    assert!(report[0].2.contains("plain.txt"), "{}", report[0].2);
}

#[tokio::test]
async fn conflicts_leave_the_tree_and_discard_cleans_up() {
    let repo = FixtureRepo::git(&[("shared.txt", "one\n")]);
    let mut h = Harness::builder(repo).settings(&bypass()).start().await;
    route_task(
        &h.model,
        "CONFLICT-TASK",
        vec![
            Script::tool("Read", json!({ "file_path": "shared.txt" })),
            Script::tool(
                "Write",
                json!({ "file_path": "shared.txt", "content": "agent\n" }),
            ),
            Script::text("changed shared.txt"),
        ],
    );
    h.model
        .push(Script::tool("Agent", isolated("CONFLICT-TASK edit it")));
    h.model.push(Script::text("done"));
    h.run_turn("change shared.txt in isolation").await;
    let agent = h.agent_started().await.agent_id;
    let pending = worktree_of(&h.agent_finished(&agent).await);
    assert_eq!(pending.state, WorktreeState::Pending);

    h.repo.write("shared.txt", "user\n");
    h.send(Command::ApplyAgentChanges {
        agent_id: agent.clone(),
    });
    worktree_state(&mut h, &agent, WorktreeState::Conflicted).await;
    assert_eq!(h.repo.read("shared.txt"), "user\n");
    assert!(Path::new(&pending.path).is_dir());

    h.send(Command::DiscardAgentChanges {
        agent_id: agent.clone(),
    });
    worktree_state(&mut h, &agent, WorktreeState::Discarded).await;
    assert!(!Path::new(&pending.path).exists());
    assert!(
        h.repo
            .run_git(&["branch", "--list", "zengine/*"])
            .trim()
            .is_empty()
    );
    assert_eq!(h.repo.read("shared.txt"), "user\n");
}
