//! Rewind: the conversation scope trims the transcript at a user message;
//! the code scope restores files from the checkpoint taken before it. Both
//! re-send the snapshot.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness};
use z_engine_protocol::{Command, Event, Message, MessageId, NoticeLevel, RewindScope, Role};
use z_engine_testkit::{FixtureRepo, Script};

fn prompts(transcript: &[Message]) -> Vec<MessageId> {
    transcript
        .iter()
        .filter(|message| message.role == Role::User && !message.is_tool_results())
        .map(|message| message.id.clone())
        .collect()
}

async fn snapshot_messages(h: &mut Harness) -> usize {
    match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => snapshot.messages.len(),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn rewind_conversation_then_code() {
    let repo = FixtureRepo::git(&[("keep.txt", "v1\n")]);
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"acceptEdits\"\n");
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    h.model.push(Script::tool(
        "Write",
        json!({ "file_path": "new.txt", "content": "created" }),
    ));
    h.model.push(Script::text("made it"));
    h.run_turn("create a file").await;
    h.model.push(Script::text("hello back"));
    h.run_turn("say hello").await;
    let transcript = h.transcript();
    assert_eq!(transcript.len(), 6);
    let turns = prompts(&transcript);
    assert_eq!(turns.len(), 2);
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::CheckpointCreated { .. })),
        2
    );

    h.send(Command::Rewind {
        message_id: turns[1].clone(),
        scope: RewindScope::Conversation,
    });
    assert_eq!(snapshot_messages(&mut h).await, 4);
    assert_eq!(h.transcript().len(), 4);
    assert!(
        h.repo.exists("new.txt"),
        "a conversation rewind leaves code alone"
    );

    h.send(Command::Rewind {
        message_id: turns[0].clone(),
        scope: RewindScope::Code,
    });
    assert_eq!(snapshot_messages(&mut h).await, 4);
    h.wait(|e| {
        matches!(
            e,
            Event::Notice { level: NoticeLevel::Info, text } if text.contains("new.txt")
        )
    })
    .await;
    assert!(
        !h.repo.exists("new.txt"),
        "the file created after the checkpoint is gone"
    );
    assert_eq!(h.repo.read("keep.txt"), "v1\n");

    h.reopen().await;
    assert_eq!(
        snapshot_messages(&mut h).await,
        4,
        "the rewind is persisted"
    );
}
