//! Fold a session log into the state the engine and GUI resume from.
//! Replay only restores state; it never repeats side effects.

use std::collections::{BTreeMap, HashMap};

use z_engine_protocol::{
    AgentId, AgentInfo, CheckRecord, CheckpointInfo, CompactionMarker, Effort, Message, MessageId,
    PermissionMode, PlanId, Question, RequestId, SessionId, TodoItem, TurnId, TurnRecord, Usage,
    decisions::TaskViewInfo,
};

use crate::record::LogRecord;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplayState {
    /// `(session id, project root, created at)` from `SessionStarted`.
    pub info: Option<(SessionId, String, u64)>,
    /// Every visible main-agent message, compacted history included (GUI).
    pub transcript: Vec<Message>,
    /// What the model sees: compaction summaries replace older history.
    pub working: Vec<Message>,
    pub compactions: Vec<CompactionMarker>,
    pub turns: Vec<TurnRecord>,
    pub todos: BTreeMap<AgentId, Vec<TodoItem>>,
    /// Latest info per agent id, in first-seen order.
    pub agents: Vec<AgentInfo>,
    pub checks: Vec<CheckRecord>,
    /// Checkpoints with their shadow commit.
    pub checkpoints: Vec<(CheckpointInfo, String)>,
    /// The permission level.
    pub mode: PermissionMode,
    pub planning: bool,
    pub model: Option<String>,
    pub effort: Option<Effort>,
    pub title: Option<String>,
    /// Every `TurnFinished` and `Usage` record in the log, including ones a
    /// conversation rewind discarded: spent money stays in the total.
    pub usage: Usage,
    pub cost_usd: f64,
    /// Started but never finished; the engine reports it as interrupted.
    pub open_turn: Option<(TurnId, MessageId)>,
    pub pending_plans: Vec<(RequestId, AgentId, String)>,
    pub pending_questions: Vec<(RequestId, AgentId, Vec<Question>)>,
    /// While a task view applies: the working set without it.
    pub full_working: Option<Vec<Message>>,
    /// Latest state of each task view, oldest first.
    pub task_views: Vec<TaskViewInfo>,
    /// The work plan the session is attached to. Plans live outside the
    /// conversation, so conversation rewinds do not change this.
    pub attached_plan: Option<PlanId>,
}

pub fn replay(records: &[LogRecord]) -> ReplayState {
    let mut state = ReplayState::default();
    for record in effective_records(records) {
        state.apply(record);
    }
    // Money already spent stays spent, so totals ignore rewinds.
    for record in records {
        match record {
            LogRecord::TurnFinished { turn } => {
                state.usage += turn.usage;
                state.cost_usd += turn.cost_usd;
            }
            LogRecord::Usage {
                usage, cost_usd, ..
            } => {
                state.usage += *usage;
                state.cost_usd += *cost_usd;
            }
            LogRecord::PlanAttached { plan_id } => state.attached_plan = Some(plan_id.clone()),
            LogRecord::PlanDetached => state.attached_plan = None,
            _ => {}
        }
    }
    state
}

/// Records left after conversation rewinds: each one cuts the list just
/// before the rewound message (dropping every kind of record after it),
/// then later records apply normally.
fn effective_records(records: &[LogRecord]) -> Vec<&LogRecord> {
    let mut effective: Vec<&LogRecord> = Vec::with_capacity(records.len());
    for record in records {
        if let LogRecord::Rewound {
            message_id,
            conversation: true,
            ..
        } = record
        {
            let target = effective.iter().position(|kept| {
                matches!(kept, LogRecord::Message { message, .. } if message.id == *message_id)
            });
            match target {
                Some(cut) => effective.truncate(cut),
                None => tracing::warn!(%message_id, "rewind target not found; ignoring it"),
            }
        }
        effective.push(record);
    }
    effective
}

impl ReplayState {
    fn apply(&mut self, record: &LogRecord) {
        match record {
            LogRecord::SessionStarted {
                session_id,
                project_root,
                model,
                mode,
                planning,
                created_at,
                ..
            } => {
                if self.info.is_none() {
                    self.info = Some((session_id.clone(), project_root.clone(), *created_at));
                    self.mode = mode.level();
                    self.planning = *planning || mode.planning();
                    self.model = Some(model.clone());
                }
            }
            LogRecord::Message { message, .. } => {
                self.transcript.push(message.clone());
                self.working.push(message.clone());
                if let Some(full) = &mut self.full_working {
                    full.push(message.clone());
                }
            }
            LogRecord::TurnStarted {
                turn_id,
                message_id,
                ..
            } => self.open_turn = Some((turn_id.clone(), message_id.clone())),
            LogRecord::TurnFinished { turn } => self.finish_turn(turn),
            LogRecord::Todos { agent_id, todos } => {
                self.todos.insert(agent_id.clone(), todos.clone());
            }
            LogRecord::PlanProposed {
                request_id,
                agent_id,
                plan,
            } => {
                self.pending_plans.retain(|(id, ..)| id != request_id);
                self.pending_plans
                    .push((request_id.clone(), agent_id.clone(), plan.clone()));
            }
            LogRecord::PlanResolved { request_id, .. } => {
                self.pending_plans.retain(|(id, ..)| id != request_id);
            }
            LogRecord::QuestionAsked {
                request_id,
                agent_id,
                questions,
            } => {
                self.pending_questions.retain(|(id, ..)| id != request_id);
                self.pending_questions.push((
                    request_id.clone(),
                    agent_id.clone(),
                    questions.clone(),
                ));
            }
            LogRecord::QuestionAnswered { request_id, .. } => {
                self.pending_questions.retain(|(id, ..)| id != request_id);
            }
            LogRecord::AgentUpdated { info } => self.update_agent(info),
            LogRecord::Check { record } => self.checks.push(record.clone()),
            LogRecord::Checkpoint { info, snapshot } => {
                self.checkpoints.push((info.clone(), snapshot.clone()));
            }
            LogRecord::Compacted { marker, summary } => self.compact(marker, summary),
            LogRecord::ModeChanged { mode, planning } => {
                self.mode = mode.level();
                self.planning = planning.unwrap_or(mode.planning());
            }
            LogRecord::PlanningChanged { planning } => self.planning = *planning,
            LogRecord::ModelChanged { model } => self.model = Some(model.clone()),
            LogRecord::EffortChanged { effort } => self.effort = *effort,
            LogRecord::Title { title } => {
                let title = title.trim();
                self.title = (!title.is_empty()).then(|| title.to_string());
            }
            LogRecord::TaskView {
                view,
                working,
                index,
            } => self.task_view(view, working, index.as_ref()),
            LogRecord::Approval { .. }
            | LogRecord::Rewound { .. }
            | LogRecord::Note { .. }
            | LogRecord::Usage { .. }
            | LogRecord::PlanAttached { .. }
            | LogRecord::PlanDetached => {}
        }
    }

    fn finish_turn(&mut self, turn: &TurnRecord) {
        if self
            .open_turn
            .as_ref()
            .is_some_and(|(turn_id, _)| *turn_id == turn.turn_id)
        {
            self.open_turn = None;
        }
        self.turns.push(turn.clone());
    }

    fn update_agent(&mut self, info: &AgentInfo) {
        match self
            .agents
            .iter_mut()
            .find(|agent| agent.agent_id == info.agent_id)
        {
            Some(agent) => *agent = info.clone(),
            None => self.agents.push(info.clone()),
        }
    }

    /// `working = [summary] + working[keep_from..]`; just the summary when
    /// `keep_from` is absent or no longer in the working set.
    fn compact(&mut self, marker: &CompactionMarker, summary: &Message) {
        let kept = marker
            .keep_from
            .as_ref()
            .and_then(|id| self.working.iter().position(|message| message.id == *id))
            .map(|start| self.working.split_off(start))
            .unwrap_or_default();
        self.working = Vec::with_capacity(kept.len() + 1);
        self.working.push(summary.clone());
        self.working.extend(kept);
        self.compactions.push(marker.clone());
        self.full_working = None;
    }

    /// Rebuilds the view's working set from the full history by id, or
    /// restores the full history.
    fn task_view(&mut self, view: &TaskViewInfo, kept: &[MessageId], index: Option<&Message>) {
        self.task_views
            .retain(|seen| seen.boundary != view.boundary);
        self.task_views.push(view.clone());
        if view.restored {
            if let Some(full) = self.full_working.take() {
                self.working = full;
            }
            return;
        }
        let full = self
            .full_working
            .take()
            .unwrap_or_else(|| std::mem::take(&mut self.working));
        let by_id: HashMap<&MessageId, &Message> = full
            .iter()
            .chain(index)
            .map(|message| (&message.id, message))
            .collect();
        self.working = kept
            .iter()
            .filter_map(|id| by_id.get(id).map(|message| (*message).clone()))
            .collect();
        self.full_working = Some(full);
    }
}
