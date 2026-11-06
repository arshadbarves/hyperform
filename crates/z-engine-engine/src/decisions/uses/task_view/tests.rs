use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use z_engine_config::{FeatureId, FeatureMode};
use z_engine_decisions::{
    Answer, Calibration, DecisionError, DecisionProvider, DecisionRequest, HybridProvider, Verdict,
};
use z_engine_protocol::{
    CallId, ContentBlock, Event, Message, MessageId, Role, ToolResultPart, ToolStatus, TurnId,
    TurnOutcome, TurnRecord, VerificationOutcome, now_ms,
};

use super::{apply_task_view_for, include_full_history};
use crate::batch::ToolCall;
use crate::decisions::seams::{annotate_result, at_turn_start};
use crate::decisions::service::DecisionService;
use crate::decisions::uses::scripted::{Events, main_run, session, traced};
use crate::run::{MainSink, TranscriptSink};
use crate::session::SessionHandle;

/// Answers from the state: the boundary is `boundary`; an exchange is
/// needed when its request says NEEDED; "always" sets a standing rule.
#[derive(Debug)]
struct ByState {
    boundary: &'static str,
    down: bool,
}

impl ByState {
    fn says(boundary: &'static str) -> Arc<Self> {
        Arc::new(Self {
            boundary,
            down: false,
        })
    }
}

#[async_trait]
impl DecisionProvider for ByState {
    fn name(&self) -> &'static str {
        "by-state"
    }

    fn revision(&self) -> String {
        "r1".into()
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        _cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        if self.down {
            return Err(DecisionError::Unavailable("refused".into()));
        }
        let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
        let answers = request.questions.iter().map(|question| {
            let proposal = match question.name.as_str() {
                "task_boundary" => Verdict::Choice(self.boundary.into()),
                "exchange_needed" => {
                    Verdict::YesNo(text(&request.state["exchange"]["request"]).contains("NEEDED"))
                }
                _ => Verdict::YesNo(text(&request.state["message"]).contains("always")),
            };
            Answer {
                question: question.name.clone(),
                proposal: Some(proposal),
                probabilities: BTreeMap::new(),
                confidence: Some(0.99),
                abstain: None,
                provider: "by-state".into(),
                latency_ms: 1,
                cached: false,
            }
        });
        Ok(answers.collect())
    }
}

fn install(handle: &SessionHandle, mode: FeatureMode, model: Arc<ByState>) {
    let modes = BTreeMap::from([(FeatureId::DecisionsTaskView, mode)]);
    let provider = Arc::new(HybridProvider::new(model, Calibration::new(0.8)));
    let service = DecisionService::new(modes, provider, None, Duration::from_millis(250));
    handle.core.decisions.replace(service);
}

fn exchange(id: &str, request: &str, path: &str, chars: usize) -> Vec<Message> {
    let call = CallId::from(id);
    let input = json!({ "file_path": path });
    vec![
        Message::user_text(request),
        Message::new(
            Role::Assistant,
            vec![ContentBlock::ToolUse {
                id: call.clone(),
                name: "Read".into(),
                input,
            }],
        ),
        Message::new(
            Role::User,
            vec![ContentBlock::tool_result(call, "x".repeat(chars), false)],
        ),
        Message::assistant_text(format!("Read {path}.")),
    ]
}

/// Five finished exchanges (about 33k tokens): a standing rule with a
/// large read of a.rs, a needed one, a large read of c.rs, two recent.
fn seed(handle: &SessionHandle, finished_at: u64) -> Vec<MessageId> {
    let root = handle.core.root.display().to_string();
    let file = |name: &str| format!("{root}/src/{name}");
    let mut history = exchange(
        "a",
        "always run the linter; read a.rs",
        &file("a.rs"),
        64_000,
    );
    history.extend(exchange("b", "NEEDED read b.rs", &file("b.rs"), 1_000));
    history.extend(exchange("c", "read c.rs", &file("c.rs"), 64_000));
    history.extend(exchange("d", "read d.rs", &file("d.rs"), 1_000));
    history.extend(exchange("e", "read e.rs", &file("e.rs"), 1_000));
    handle.core.with_state(|state| {
        state.transcript.extend(history.iter().cloned());
        state.working.extend(history.iter().cloned());
        state.turns.push(TurnRecord {
            turn_id: TurnId::new(),
            message_id: history[0].id.clone(),
            outcome: TurnOutcome::Completed,
            verification: VerificationOutcome::NotApplicable,
            usage: Default::default(),
            cost_usd: 0.0,
            started_at: finished_at,
            finished_at,
        });
    });
    history.iter().map(|message| message.id.clone()).collect()
}

/// The turn's opening: turn-start seam, the message saved, the view applied.
async fn send(handle: &SessionHandle, text: &str) -> Message {
    at_turn_start(&main_run(handle), text).await;
    let message = Message::user_text(text);
    let sink = MainSink::new(Arc::clone(&handle.core), None);
    sink.append(&message, false).unwrap();
    apply_task_view_for(&handle.core, &message);
    message
}

fn working(handle: &SessionHandle) -> Vec<Message> {
    handle.core.with_state(|state| state.working.clone())
}

fn has_result(working: &[Message], id: &str) -> bool {
    working
        .iter()
        .flat_map(|message| &message.content)
        .any(|block| matches!(block, ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id.as_str() == id))
}

fn applied(events: &Events) -> usize {
    let events = events.lock().unwrap();
    let views = events
        .iter()
        .filter(|e| matches!(e, Event::TaskViewApplied { .. }));
    views.count()
}

fn outcomes(handle: &SessionHandle, outcome: &str) -> usize {
    let records = handle.core.decisions.trace().recent(100);
    records.iter().filter(|r| r.outcome == outcome).count()
}

#[tokio::test]
async fn off_keeps_the_history_whole() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let ids = seed(&handle, now_ms());
    send(&handle, "now something else").await;
    assert_eq!(working(&handle).len(), ids.len() + 1);
    assert_eq!(applied(&events), 0);
}

#[tokio::test]
async fn on_sets_aside_unneeded_exchanges_and_pins_standing_rules() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FeatureMode::On, ByState::says("unrelated"));
    let ids = seed(&handle, now_ms());
    let message = send(&handle, "now something else").await;
    let view = working(&handle);
    assert!(!has_result(&view, "a") && !has_result(&view, "c"));
    assert!(has_result(&view, "b") && has_result(&view, "d") && has_result(&view, "e"));
    assert!(
        view.iter().any(|m| m.id == ids[0]),
        "the rule's message stays"
    );
    assert_eq!(view.last().unwrap().id, message.id);
    let full = handle.core.with_state(|state| state.full_working.clone());
    assert_eq!(full.unwrap().len(), ids.len() + 1);
    assert_eq!(applied(&events), 1);
    let info = handle.core.with_state(|state| state.task_views.clone());
    assert_eq!(
        (info[0].set_aside, info[0].boundary.clone()),
        (2, message.id)
    );
    assert_eq!(outcomes(&handle, "set aside"), 2);
    assert_eq!(outcomes(&handle, "pinned"), 1);
    assert_eq!(outcomes(&handle, "new task"), 1);
}

#[tokio::test]
async fn files_the_new_request_names_stay() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    install(&handle, FeatureMode::On, ByState::says("related"));
    seed(&handle, now_ms());
    send(&handle, "now refactor src/c.rs").await;
    let view = working(&handle);
    assert!(!has_result(&view, "a") && has_result(&view, "c"));
}

#[tokio::test]
async fn a_continuing_message_keeps_the_history_whole() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FeatureMode::On, ByState::says("continue"));
    let ids = seed(&handle, now_ms());
    send(&handle, "and also the tests").await;
    assert_eq!(working(&handle).len(), ids.len() + 1);
    assert_eq!(applied(&events), 0);
}

#[tokio::test]
async fn idling_past_the_cache_lifetime_is_a_boundary_by_rule() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FeatureMode::On, ByState::says("continue"));
    seed(&handle, now_ms() - 10 * 60 * 1_000);
    send(&handle, "and also the tests").await;
    assert!(!has_result(&working(&handle), "c"));
    assert_eq!(applied(&events), 1);
}

#[tokio::test]
async fn shadow_records_but_keeps_the_history_whole() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FeatureMode::Shadow, ByState::says("unrelated"));
    let ids = seed(&handle, now_ms());
    send(&handle, "now something else").await;
    assert!(traced(&handle, |r| r.shadow && r.outcome == "set aside").await);
    apply_task_view_for(&handle.core, &working(&handle).pop().unwrap());
    assert_eq!(working(&handle).len(), ids.len() + 1);
    assert_eq!(applied(&events), 0);
}

#[tokio::test]
async fn an_unavailable_model_keeps_the_history_whole() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    let model = Arc::new(ByState {
        boundary: "unrelated",
        down: true,
    });
    install(&handle, FeatureMode::On, model);
    let ids = seed(&handle, now_ms() - 10 * 60 * 1_000);
    send(&handle, "now something else").await;
    assert_eq!(working(&handle).len(), ids.len() + 1);
    assert_eq!(applied(&events), 0);
}

#[tokio::test]
async fn including_full_history_restores_it_and_later_messages_join_both() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, events) = session(dir.path()).await;
    install(&handle, FeatureMode::On, ByState::says("unrelated"));
    let ids = seed(&handle, now_ms());
    send(&handle, "now something else").await;
    let reply = Message::assistant_text("done");
    let sink = MainSink::new(Arc::clone(&handle.core), None);
    sink.append(&reply, false).unwrap();
    include_full_history(&handle.core);
    let restored = working(&handle);
    assert_eq!(restored.len(), ids.len() + 2);
    assert_eq!(restored.last().unwrap().id, reply.id);
    let views = handle.core.with_state(|state| state.task_views.clone());
    assert!(views[0].restored);
    assert_eq!(applied(&events), 2);
    assert_eq!(outcomes(&handle, "full history restored"), 1);
    include_full_history(&handle.core);
    assert_eq!(applied(&events), 2, "nothing left to restore");
}

#[tokio::test]
async fn reading_a_saved_exchange_is_a_readback() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    install(&handle, FeatureMode::On, ByState::says("unrelated"));
    seed(&handle, now_ms());
    send(&handle, "now something else").await;
    let index = working(&handle)
        .iter()
        .flat_map(|m| &m.content)
        .find_map(|block| match block {
            ContentBlock::Text { text } if text.contains("full text at ") => Some(text.clone()),
            _ => None,
        })
        .unwrap();
    let after = index.split("full text at ").nth(1).unwrap();
    let path: String = after.chars().take_while(|ch| !ch.is_whitespace()).collect();
    assert!(std::path::Path::new(&path).exists(), "{path}");
    let call = ToolCall {
        id: CallId::from("back"),
        name: "Read".into(),
        input: json!({ "file_path": path }),
    };
    let mut content = vec![ToolResultPart::Text { text: "x".into() }];
    annotate_result(&main_run(&handle), &call, ToolStatus::Ok, &mut content).await;
    assert_eq!(content.len(), 1, "no note for the model");
    assert_eq!(outcomes(&handle, "readback"), 1);
}
