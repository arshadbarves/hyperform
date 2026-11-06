//! The session actor: owns the command channel and serializes commands. It
//! never waits on the model or a tool: turns and manual compactions run as
//! activities in their own tasks and report back when they end, so
//! steering, approvals, mode changes and cancel stay responsive.

use std::sync::Arc;
use std::time::Duration;

use hyperform_protocol::{Command, MessageId, NoticeLevel, RewindScope, TurnOutcome};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::compaction::compact_now;
use super::control;
use super::handle::{ActorMsg, SessionHandle};
use super::meta::write_meta;
use super::prompt::TurnInput;
use super::reload::reload;
use super::rewind::rewind;
use super::shell::spawn_shell;
use super::slash::{Slash, run_command};
use super::trust::trust_workspace;
use super::turn::run_turn;
use crate::decisions::{include_full_history, resolve_suggestion};
use crate::hooks::{HookEvent, HookInput, run_hooks};
use crate::orchestration::{apply_command, discard_command};
use crate::session::SessionCore;
use crate::work;

const COMMAND_CAPACITY: usize = 256;
/// How long shutdown waits for a cancelled activity to record its end.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

struct Activity {
    cancel: CancellationToken,
    task: JoinHandle<()>,
}

/// Tells the actor an activity ended (whether it was cancelled); sent on
/// drop too, so a panicking task still frees the session.
struct Ended {
    tx: mpsc::UnboundedSender<bool>,
    token: CancellationToken,
    sent: bool,
}

impl Ended {
    fn finish(mut self, cancelled: bool) {
        self.send(cancelled);
    }

    fn send(&mut self, cancelled: bool) {
        if std::mem::replace(&mut self.sent, true) {
            return;
        }
        if self.tx.send(cancelled).is_err() {
            tracing::debug!("activity ended after its actor stopped");
        }
    }
}

impl Drop for Ended {
    fn drop(&mut self) {
        let cancelled = self.token.is_cancelled();
        self.send(cancelled);
    }
}

pub(crate) fn spawn_actor(core: Arc<SessionCore>) -> SessionHandle {
    let (tx, commands) = mpsc::channel(COMMAND_CAPACITY);
    let (ended, ended_rx) = mpsc::unbounded_channel();
    let actor = Actor {
        core: Arc::clone(&core),
        ended,
        activity: None,
        interrupt_text: None,
    };
    let task = tokio::spawn(actor.run(commands, ended_rx));
    SessionHandle::new(core, tx, task)
}

struct Actor {
    core: Arc<SessionCore>,
    ended: mpsc::UnboundedSender<bool>,
    activity: Option<Activity>,
    /// Sent as a new turn once the interrupted one has ended.
    interrupt_text: Option<String>,
}

impl Actor {
    async fn run(
        mut self,
        mut commands: mpsc::Receiver<ActorMsg>,
        mut ended: mpsc::UnboundedReceiver<bool>,
    ) {
        let (reason, done) = loop {
            tokio::select! {
                Some(cancelled) = ended.recv() => self.on_ended(cancelled),
                message = commands.recv() => match message {
                    Some(ActorMsg::Command(Command::Shutdown)) => break ("shutdown", None),
                    Some(ActorMsg::Command(command)) => self.handle(command).await,
                    Some(ActorMsg::Reload) => reload(&self.core).await,
                    Some(ActorMsg::Close { reason, done }) => break (reason, Some(done)),
                    None => break ("closed", None),
                },
            }
        };
        self.shutdown(reason).await;
        if let Some(done) = done {
            if done.send(()).is_err() {
                tracing::debug!(session = %self.core.id, "close waiter gone");
            }
        }
    }

    async fn handle(&mut self, command: Command) {
        let core = &self.core;
        match command {
            Command::Submit { text, attachments } => self.submit(TurnInput::new(text, attachments)),
            Command::Steer { text } => self.submit(TurnInput::text(text)),
            Command::Interrupt { text } => self.interrupt(text),
            Command::Cancel => self.cancel(),
            Command::ResolveApproval {
                request_id,
                decision,
            } => control::resolve_approval(core, &request_id, decision),
            Command::AnswerQuestion {
                request_id,
                answers,
            } => control::answer_question(core, &request_id, answers),
            Command::ResolvePlan {
                request_id,
                decision,
            } => control::resolve_plan(core, &request_id, decision),
            Command::SetMode { mode } => control::set_mode(core, mode),
            Command::SetPlanning { planning } => control::set_planning(core, planning),
            Command::SetModel { model } => control::set_model(core, model),
            Command::SetEffort { effort } => control::set_effort(core, effort),
            Command::Compact { instructions } => self.compact(instructions),
            Command::Rewind { message_id, scope } => self.rewind(&message_id, scope).await,
            Command::KillJob { job_id } => control::kill_job(core, job_id),
            Command::ApplyAgentChanges { agent_id } => apply_command(core, agent_id),
            Command::DiscardAgentChanges { agent_id } => discard_command(core, agent_id),
            Command::RunCommand { name, args } => match run_command(core, &name, &args).await {
                Slash::Compact(instructions) => self.compact(instructions),
                Slash::Prompt(call) if self.activity.is_none() => {
                    self.start_turn(TurnInput::command(call));
                }
                Slash::Prompt(call) => control::command_waits(core, &call.name),
                Slash::Handled => {}
            },
            Command::Shell { command } => spawn_shell(Arc::clone(core), command),
            Command::EditQueue { queued } => control::edit_queue(core, queued),
            Command::ReloadExtensions => reload(core).await,
            Command::TrustWorkspace { trusted } => trust_workspace(core, trusted).await,
            Command::ResolveSuggestion {
                suggestion_id,
                accepted,
            } => resolve_suggestion(core, &suggestion_id, accepted),
            Command::IncludeFullHistory => include_full_history(core),
            Command::EditWork { plan_id, edit } => work::edit(core, &plan_id, edit),
            Command::AttachPlan { plan_id } => work::attach_command(core, &plan_id).await,
            Command::DetachPlan => work::detach_command(core),
            Command::Shutdown => {}
        }
    }

    /// Busy: the text is queued as steering for the next round boundary.
    fn submit(&mut self, input: TurnInput) {
        if self.activity.is_none() {
            self.start_turn(input);
            return;
        }
        if !input.attachments.is_empty() {
            self.core.events.notice(
                NoticeLevel::Warn,
                "Attachments are not queued while the agent works; send them again after this turn.",
            );
        }
        control::steer(&self.core, input.text);
    }

    fn interrupt(&mut self, text: Option<String>) {
        let text = text.filter(|text| !text.trim().is_empty());
        match &self.activity {
            Some(activity) => {
                self.interrupt_text = text;
                activity.cancel.cancel();
            }
            None => {
                if let Some(text) = text {
                    self.start_turn(TurnInput::text(text));
                }
            }
        }
    }

    fn cancel(&mut self) {
        if let Some(activity) = &self.activity {
            activity.cancel.cancel();
        }
    }

    fn start_turn(&mut self, input: TurnInput) {
        let core = Arc::clone(&self.core);
        self.start(move |token, ended| async move {
            let outcome = run_turn(core, input, token).await;
            ended.finish(matches!(outcome, Some(TurnOutcome::Cancelled)));
        });
    }

    fn compact(&mut self, instructions: Option<String>) {
        if self.activity.is_some() {
            self.core.events.notice(
                NoticeLevel::Warn,
                "Compaction waits for an idle session; try again when this turn ends.",
            );
            return;
        }
        let core = Arc::clone(&self.core);
        self.start(move |token, ended| async move {
            compact_now(&core, instructions, &token).await;
            ended.finish(token.is_cancelled());
        });
    }

    fn start<F, Fut>(&mut self, work: F)
    where
        F: FnOnce(CancellationToken, Ended) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let cancel = self.core.cancel.child_token();
        let ended = Ended {
            tx: self.ended.clone(),
            token: cancel.clone(),
            sent: false,
        };
        self.core.status.begin();
        let task = tokio::spawn(work(cancel.clone(), ended));
        self.activity = Some(Activity { cancel, task });
    }

    fn on_ended(&mut self, cancelled: bool) {
        self.activity = None;
        self.core.status.end();
        if let Some(text) = self.interrupt_text.take() {
            self.start_turn(TurnInput::text(text));
            return;
        }
        let queued = self.core.with_state(|state| !state.queue.is_empty());
        if queued && !cancelled {
            self.start_turn(TurnInput::default());
        }
    }

    async fn rewind(&mut self, message_id: &MessageId, scope: RewindScope) {
        if self.activity.is_some() {
            self.core.events.notice(
                NoticeLevel::Warn,
                "Rewind needs an idle session; stop the current turn first.",
            );
            return;
        }
        self.core.status.begin();
        if let Err(error) = rewind(&self.core, message_id, scope).await {
            self.core
                .events
                .notice(NoticeLevel::Warn, format!("Rewind failed: {error}"));
        }
        self.core.status.end();
    }

    async fn shutdown(&mut self, reason: &str) {
        work::closing(&self.core);
        if let Some(activity) = self.activity.take() {
            activity.cancel.cancel();
            match tokio::time::timeout(STOP_TIMEOUT, activity.task).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::error!(session = %self.core.id, %error, "activity failed")
                }
                Err(_) => tracing::warn!(session = %self.core.id, "activity did not stop in time"),
            }
            self.core.status.end();
        }
        let core = &self.core;
        core.broker.withdraw_all();
        let input = HookInput::new().with("reason", json!(reason));
        let hooks_cancel = CancellationToken::new();
        run_hooks(
            &core.hook_env(),
            &core.events,
            HookEvent::SessionEnd,
            input,
            &hooks_cancel,
        )
        .await;
        core.agents.close_slots();
        core.jobs.kill_all().await;
        core.cancel.cancel();
        tokio::join!(core.mcp.shutdown(), core.lsp.shutdown());
        write_meta(core);
        core.journal.sync_or_report();
    }
}
