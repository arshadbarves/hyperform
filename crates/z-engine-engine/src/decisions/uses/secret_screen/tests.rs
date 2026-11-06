use std::time::Duration;

use serde_json::json;
use z_engine_config::{FeatureId, FeatureMode};
use z_engine_protocol::{
    ApprovalDecision, CallId, ContentBlock, Event, Message, MessageId, PermissionMode, Role,
};

use super::detect::{entropy, scan};
use crate::decisions::seams::screen_request;
use crate::decisions::uses::scripted::{
    Events, Scripted, install, main_run, notices, session, session_with, traced,
};
use crate::run::{MainSink, TranscriptSink};
use crate::session::SessionHandle;

const FEATURE: FeatureId = FeatureId::DecisionsSecretScreen;
const AWS: &str = "AKIAIOSFODNN7REALKEY";
const MASKED: &str = "[secret withheld by the user: AWS access key]";

/// A Bash call and its result `output`.
fn exchange(id: &str, output: &str) -> Vec<Message> {
    let message = |role, content| Message {
        id: MessageId::new(),
        role,
        content: vec![content],
        created_at: 0,
    };
    let call = ContentBlock::ToolUse {
        id: CallId::from(id),
        name: "Bash".into(),
        input: json!({ "command": "env" }),
    };
    let result = ContentBlock::tool_result(CallId::from(id), output, false);
    vec![message(Role::Assistant, call), message(Role::User, result)]
}

fn result_text(working: &[Message]) -> String {
    let texts = working
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|block| match block {
            ContentBlock::ToolResult { content, .. } => Some(format!("{content:?}")),
            _ => None,
        });
    texts.collect::<Vec<_>>().join("\n")
}

/// Screens `working` as the main agent's next request, answering the one
/// approval card (if any) with `answer`; `None` leaves it unanswered.
async fn screen(
    handle: &SessionHandle,
    events: &Events,
    working: Vec<Message>,
    answer: Option<ApprovalDecision>,
) -> (Vec<Message>, usize) {
    let sink = MainSink::new(handle.core.clone(), None);
    sink.set_working(working.clone());
    let ctx = main_run(handle);
    let asked = || {
        let events = events.lock().unwrap();
        let cards = events.iter().filter_map(|event| match event {
            Event::ApprovalRequested { request } => Some(request.request_id.clone()),
            _ => None,
        });
        cards.collect::<Vec<_>>()
    };
    let before = asked().len();
    let reply = async {
        let Some(decision) = answer else {
            tokio::time::sleep(Duration::from_millis(300)).await;
            ctx.cancel.cancel();
            return;
        };
        for _ in 0..100 {
            if let Some(id) = asked().get(before) {
                let _ = handle.core.broker.resolve_approval(id, decision, |_, _| {});
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    let (screened, ()) = tokio::join!(screen_request(&ctx, &sink, working), reply);
    assert_eq!(screened, sink.working(), "the screened transcript is kept");
    (screened, asked().len() - before)
}

#[test]
fn detectors_find_key_formats_and_leave_placeholders_alone() {
    let found = scan(&format!(
        "AWS_ACCESS_KEY_ID={AWS}\nGITHUB=ghp_{}\nDB=postgres://app:s3cr3tPass@db/app\n\
         api_token = 'q8Zr2LmX9vTk4WbN7yHs'\npassword = ${{DB_PASSWORD}}\n\
         secret: your-secret-here\ntoken = config.token\nDB_PASSWORD=hunter2hunter2",
        "a".repeat(36)
    ));
    let kinds: Vec<&str> = found.secrets.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [
            "AWS access key",
            "GitHub token",
            "password in a URL",
            "credential-like value"
        ]
    );
    assert_eq!(found.candidates.len(), 1, "{:?}", found.candidates);
    assert_eq!(found.candidates[0].1, "hunter2hunter2");
    assert!(entropy("aaaa") < 0.01 && entropy("q8Zr2LmX9vTk4WbN7yHs") > 3.5);
    let quiet = scan("let token: String = String::new();\npassword = \"\"\nfn api_key() {}");
    assert!(quiet.secrets.is_empty() && quiet.candidates.is_empty());
}

#[tokio::test]
async fn a_declined_secret_is_masked_in_this_and_later_requests() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FEATURE, FeatureMode::On, Scripted::default());
    let output = format!("KEY={AWS}\nok");
    let deny = ApprovalDecision::Deny { feedback: None };
    let (masked, cards) = screen(&handle, &events, exchange("c1", &output), Some(deny)).await;
    assert_eq!(cards, 1);
    let text = result_text(&masked);
    assert!(!text.contains(AWS) && text.contains(MASKED), "{text}");

    let mut later = masked.clone();
    later.extend(exchange("c2", &format!("again {AWS}")));
    let (masked, cards) = screen(&handle, &events, later, None).await;
    assert_eq!(cards, 0, "a withheld value is masked without asking again");
    assert!(!result_text(&masked).contains(AWS));
}

#[tokio::test]
async fn an_allowed_secret_is_sent_and_not_asked_about_again() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FEATURE, FeatureMode::On, Scripted::default());
    let working = exchange("c1", &format!("KEY={AWS}"));
    let allow = Some(ApprovalDecision::AllowOnce);
    let (sent, cards) = screen(&handle, &events, working.clone(), allow).await;
    assert_eq!((sent, cards), (working.clone(), 1));
    let mut later = working.clone();
    later.extend(exchange("c2", AWS));
    let (sent, cards) = screen(&handle, &events, later.clone(), None).await;
    assert_eq!((sent, cards), (later, 0));
}

#[tokio::test]
async fn an_unanswered_card_withholds_when_the_run_is_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FEATURE, FeatureMode::On, Scripted::default());
    let (masked, cards) = screen(&handle, &events, exchange("c1", AWS), None).await;
    assert_eq!(cards, 1);
    assert!(!result_text(&masked).contains(AWS));
    assert!(
        handle.core.broker.snapshot().0.is_empty(),
        "the card is withdrawn"
    );
}

#[tokio::test]
async fn off_and_shadow_send_everything_without_asking() {
    for mode in [FeatureMode::Off, FeatureMode::Shadow] {
        let dir = tempfile::tempdir().unwrap();
        let (handle, events) = session(dir.path()).await;
        install(&handle, FEATURE, mode, Scripted::default());
        let working = exchange("c1", AWS);
        let (sent, cards) = screen(&handle, &events, working.clone(), None).await;
        assert_eq!((sent, cards), (working, 0), "{mode:?}");
        if mode == FeatureMode::Shadow {
            let print = super::ledger::fingerprint(AWS);
            let shadow = |r: &z_engine_decisions::DecisionRecord| {
                r.shadow && r.outcome == "flagged" && r.input_fingerprint == print
            };
            assert!(traced(&handle, shadow).await);
        }
    }
}

#[tokio::test]
async fn bypass_mode_only_posts_a_notice() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FEATURE, FeatureMode::On, Scripted::default());
    handle
        .core
        .with_state(|state| state.mode = PermissionMode::Bypass);
    let working = exchange("c1", AWS);
    let (sent, cards) = screen(&handle, &events, working.clone(), None).await;
    assert_eq!((sent, cards), (working, 0));
    assert!(
        notices(&events)
            .iter()
            .any(|n| n.contains("Bypass mode sends them"))
    );
}

#[tokio::test]
async fn the_model_judges_assignments_only_when_its_endpoint_is_local() {
    let line = "DB_PASSWORD=hunter2hunter2";
    let model = || Scripted::default().yes("secret_candidate", true);
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FEATURE, FeatureMode::On, model());
    let deny = ApprovalDecision::Deny { feedback: None };
    let (masked, cards) = screen(&handle, &events, exchange("c1", line), Some(deny)).await;
    assert_eq!(cards, 1);
    assert!(result_text(&masked).contains("possible credential"));

    for (settings, model) in [
        ("schema = 2\n[decisions]\nallow_remote = true\n", model()),
        ("schema = 2\n", Scripted::down()),
        ("schema = 2\n", Scripted::default()),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (handle, events) = session_with(dir.path(), Some(settings)).await;
        install(&handle, FEATURE, FeatureMode::On, model);
        let working = exchange("c1", line);
        let (sent, cards) = screen(&handle, &events, working.clone(), None).await;
        assert_eq!((sent, cards), (working, 0), "{settings}");
    }
}
