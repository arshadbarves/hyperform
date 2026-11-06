//! v1 events -> v2 records. v1 stored no ids or timestamps: message and turn
//! ids are fresh, and times are spread evenly between the session's
//! creation (its ULID) and the file's last modification.
//!
//! Tool rounds: the results following an assistant message become one user
//! message of `ToolResult` blocks in call order, even when other events
//! (task reports, titles) sit between them. A trailing round that never got
//! all its results is dropped, as v1 replay truncated it. An unanswered call
//! followed by later messages (v1 records no results after an aborted
//! approval) gets an error result so the working set stays valid.

use hyperform_protocol::{
    CallId, ContentBlock, Message, MessageId, PermissionMode, Role, SessionId, TurnId, TurnOutcome,
    TurnRecord, Usage, VerificationOutcome,
};

use super::mapping::{
    fallback_title, image_source, persisted_title, report_note, tool_input, tool_name, turn_outcome,
};
use super::rounds::{Round, unfinished_tail};
use super::v1::{V1Event, V1TaskReport, V1ToolCall};
use crate::record::{LogRecord, SESSION_SCHEMA};

#[derive(Debug)]
pub(super) struct Source<'a> {
    pub(super) session_id: &'a SessionId,
    pub(super) events: &'a [V1Event],
    pub(super) created_at: u64,
    pub(super) updated_at: u64,
    /// Unreadable v1 lines, recorded as a note for the audit trail.
    pub(super) skipped_lines: usize,
}

pub(super) fn convert(source: &Source<'_>) -> Vec<LogRecord> {
    Converter::new(source).run()
}

#[derive(Debug)]
struct OpenTurn {
    turn_id: TurnId,
    message_id: MessageId,
    started_at: u64,
}

#[derive(Debug)]
struct Converter<'a> {
    events: &'a [V1Event],
    created_at: u64,
    updated_at: u64,
    out: Vec<LogRecord>,
    round: Option<Round>,
    turn: Option<OpenTurn>,
    /// Latest report of the current turn; v1 rewrote it after every tool.
    report: Option<&'a V1TaskReport>,
    /// Index of the trailing assistant message whose round is unfinished.
    unfinished_tail: Option<usize>,
    /// Past the unfinished tail: its results are dropped with it.
    discarding: bool,
    titled: bool,
    first_prompt: Option<&'a str>,
}

impl<'a> Converter<'a> {
    fn new(source: &Source<'a>) -> Self {
        let mut out = vec![session_started(source)];
        if source.skipped_lines > 0 {
            out.push(LogRecord::Note {
                text: format!(
                    "v1 import skipped {} unreadable line(s)",
                    source.skipped_lines
                ),
            });
        }
        Self {
            events: source.events,
            created_at: source.created_at,
            updated_at: source.updated_at,
            out,
            round: None,
            turn: None,
            report: None,
            unfinished_tail: unfinished_tail(source.events),
            discarding: false,
            titled: false,
            first_prompt: None,
        }
    }

    fn run(mut self) -> Vec<LogRecord> {
        let events = self.events;
        for (index, event) in events.iter().enumerate() {
            let at = self.time_of(index);
            match event {
                V1Event::Meta { .. } | V1Event::Ack => {}
                V1Event::UserMsg { text, images } => self.user(text, images, at),
                V1Event::AssistantMsg { .. } if self.unfinished_tail == Some(index) => {
                    self.discarding = true;
                }
                V1Event::AssistantMsg {
                    content,
                    tool_calls,
                } => self.assistant(content.as_deref(), tool_calls, at),
                V1Event::ToolResult {
                    tool_call_id,
                    content,
                } => self.tool_result(tool_call_id, content, at),
                V1Event::Note { text } => self.out.push(LogRecord::Note { text: text.clone() }),
                V1Event::Title { text } => self.title(text),
                V1Event::TurnEnd { outcome } => {
                    self.flush_round(at);
                    self.close_turn(turn_outcome(outcome), at);
                }
                V1Event::TaskUpdated { report } => self.report = Some(report),
            }
        }
        let at = self.updated_at;
        self.flush_round(at);
        self.close_turn(TurnOutcome::Interrupted, at);
        self.flush_report();
        if !self.titled {
            if let Some(title) = self.first_prompt.and_then(fallback_title) {
                self.out.push(LogRecord::Title { title });
            }
        }
        self.out
    }

    fn user(&mut self, text: &'a str, images: &[String], at: u64) {
        self.flush_round(at);
        self.close_turn(TurnOutcome::Interrupted, at);
        self.flush_report();
        if self.first_prompt.is_none() {
            self.first_prompt = Some(text);
        }
        let mut content = Vec::new();
        if !text.is_empty() || images.is_empty() {
            content.push(ContentBlock::text(text));
        }
        content.extend(images.iter().map(|url| ContentBlock::Image {
            source: image_source(url),
        }));
        let turn_id = TurnId::new();
        let message_id = self.push_message(Role::User, content, at, Some(turn_id.clone()));
        self.out.push(LogRecord::TurnStarted {
            turn_id: turn_id.clone(),
            message_id: message_id.clone(),
            started_at: at,
        });
        self.turn = Some(OpenTurn {
            turn_id,
            message_id,
            started_at: at,
        });
    }

    fn assistant(&mut self, text: Option<&str>, calls: &[V1ToolCall], at: u64) {
        self.flush_round(at);
        let mut content: Vec<ContentBlock> = text
            .filter(|text| !text.is_empty())
            .map(ContentBlock::text)
            .into_iter()
            .collect();
        content.extend(calls.iter().map(|call| ContentBlock::ToolUse {
            id: CallId::from(call.id.as_str()),
            name: tool_name(&call.name).to_string(),
            input: tool_input(&call.arguments),
        }));
        if content.is_empty() {
            return;
        }
        let turn_id = self.current_turn();
        self.push_message(Role::Assistant, content, at, turn_id);
        if !calls.is_empty() {
            self.round = Some(Round::new(calls));
        }
    }

    fn tool_result(&mut self, call_id: &str, content: &str, at: u64) {
        if self.discarding {
            return;
        }
        let Some(round) = self.round.as_mut() else {
            tracing::warn!(
                call_id,
                "dropping v1 tool result without a pending tool call"
            );
            return;
        };
        if !round.answer(call_id, content) {
            tracing::warn!(
                call_id,
                "dropping v1 tool result that matches no pending call"
            );
            return;
        }
        if round.is_complete() {
            self.flush_round(at);
        }
    }

    fn title(&mut self, text: &str) {
        if self.titled {
            return;
        }
        // v1 displayed the first persisted title.
        if let Some(title) = persisted_title(text) {
            self.titled = true;
            self.out.push(LogRecord::Title { title });
        }
    }

    fn flush_round(&mut self, at: u64) {
        if let Some(round) = self.round.take() {
            let turn_id = self.current_turn();
            self.push_message(Role::User, round.into_blocks(), at, turn_id);
        }
    }

    fn close_turn(&mut self, outcome: TurnOutcome, at: u64) {
        let Some(turn) = self.turn.take() else {
            return;
        };
        self.out.push(LogRecord::TurnFinished {
            turn: TurnRecord {
                turn_id: turn.turn_id,
                message_id: turn.message_id,
                outcome,
                verification: VerificationOutcome::NotApplicable,
                usage: Usage::default(),
                cost_usd: 0.0,
                started_at: turn.started_at,
                finished_at: at,
            },
        });
    }

    fn flush_report(&mut self) {
        if let Some(report) = self.report.take() {
            self.out.push(LogRecord::Note {
                text: report_note(report),
            });
        }
    }

    fn push_message(
        &mut self,
        role: Role,
        content: Vec<ContentBlock>,
        at: u64,
        turn_id: Option<TurnId>,
    ) -> MessageId {
        let message = Message {
            id: MessageId::new(),
            role,
            content,
            created_at: at,
        };
        let id = message.id.clone();
        self.out.push(LogRecord::Message { message, turn_id });
        id
    }

    fn current_turn(&self) -> Option<TurnId> {
        self.turn.as_ref().map(|turn| turn.turn_id.clone())
    }

    fn time_of(&self, index: usize) -> u64 {
        let span = u128::from(self.updated_at.saturating_sub(self.created_at));
        let steps = self.events.len().saturating_sub(1).max(1) as u128;
        let offset = span * index as u128 / steps;
        self.created_at
            .saturating_add(u64::try_from(offset).unwrap_or(u64::MAX))
    }
}

fn session_started(source: &Source<'_>) -> LogRecord {
    let meta = source.events.iter().find_map(|event| match event {
        V1Event::Meta {
            model,
            project_root,
        } => Some((model.as_str(), project_root.as_str())),
        _ => None,
    });
    let (model, project_root) = meta.unwrap_or_default();
    LogRecord::SessionStarted {
        schema: SESSION_SCHEMA,
        session_id: source.session_id.clone(),
        project_root: project_root.to_string(),
        model: model.to_string(),
        mode: PermissionMode::Default.into(),
        planning: false,
        created_at: source.created_at,
    }
}
