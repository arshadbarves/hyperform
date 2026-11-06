//! Attachments on a submitted prompt: images become image blocks and
//! files are attached as context.

mod support;

use support::{Harness, last_user_text};
use z_engine_protocol::{Attachment, Command, ContentBlock};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn images_and_files_are_attached() {
    let repo = FixtureRepo::git(&[("notes.md", "the launch is on friday\n")]);
    let mut h = Harness::start(repo).await;
    h.model.push(Script::text("got them"));
    h.send(Command::Submit {
        text: "see attached".into(),
        attachments: vec![
            Attachment::Image {
                media_type: "image/png".into(),
                data: "iVBORw0KGgo=".into(),
            },
            Attachment::File {
                path: "notes.md".into(),
            },
        ],
    });
    h.turn_finished().await;
    let request = h.main_requests().pop().unwrap();
    let prompt = &request.messages[0];
    assert!(
        prompt
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::Image { .. }))
    );
    let text = last_user_text(&request);
    assert!(
        text.contains("see attached") && text.contains("the launch is on friday"),
        "{text}"
    );
}
