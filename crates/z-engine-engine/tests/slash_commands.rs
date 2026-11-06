//! Built-in commands answered without the model: context, cost, status,
//! remember (which also reloads instructions), and unknown commands with
//! suggestions of close matches.

mod support;

use support::Harness;
use z_engine_protocol::{Command, Event, NoticeLevel};
use z_engine_testkit::{FixtureRepo, Script};

fn run(h: &Harness, name: &str, args: &str) {
    h.send(Command::RunCommand {
        name: name.into(),
        args: args.into(),
    });
}

async fn output(h: &mut Harness, wanted: &str) -> String {
    let wanted = wanted.to_string();
    match h
        .wait(move |e| matches!(e, Event::CommandOutput { name, .. } if *name == wanted))
        .await
    {
        Event::CommandOutput { markdown, .. } => markdown,
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn informational_commands_and_remember() {
    let mut h = Harness::start(FixtureRepo::empty()).await;

    run(&h, "context", "");
    let context = output(&mut h, "context").await;
    assert!(context.contains("Tool definitions") && context.contains("Total"));
    match h.expect(|e| matches!(e, Event::ContextReport { .. })).await {
        Event::ContextReport { breakdown } => {
            assert!(breakdown.system > 0 && breakdown.tools > 0);
            assert_eq!(breakdown.limit, 128_000);
        }
        other => panic!("{other:?}"),
    }
    run(&h, "cost", "");
    assert!(output(&mut h, "cost").await.contains("Session cost"));
    run(&h, "status", "");
    assert!(output(&mut h, "status").await.contains("test-model"));

    run(&h, "remember", "project Always use tabs");
    let saved = output(&mut h, "remember").await;
    assert!(saved.contains("- Always use tabs"), "{saved}");
    assert!(h.repo.read("AGENTS.md").contains("- Always use tabs"));
    h.model.push(Script::text("ok"));
    h.run_turn("hello").await;
    let request = h.main_requests().pop().unwrap();
    assert!(
        request.system_text().contains("Always use tabs"),
        "instructions reloaded"
    );

    run(&h, "frobnicate", "");
    h.wait(|e| {
        matches!(
            e,
            Event::Notice { level: NoticeLevel::Info, text }
                if text.contains("/frobnicate is not a known command.") && !text.contains("Did you mean")
        )
    })
    .await;
    run(&h, "reveiw", "");
    h.wait(|e| {
        matches!(
            e,
            Event::Notice { level: NoticeLevel::Info, text }
                if text.contains("/reveiw") && text.contains("Did you mean /review")
        )
    })
    .await;
}
