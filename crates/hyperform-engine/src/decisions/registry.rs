//! The `DecisionUse` trait and `USES`, the registered uses. A use names its
//! feature and the seams it joins, and overrides only those seams' methods;
//! the defaults give no advice, which is exactly today's behavior.

use std::path::PathBuf;

use async_trait::async_trait;
use hyperform_config::FeatureId;
use hyperform_context::ClearTarget;
use hyperform_protocol::decisions::{TurnTone, UncheckedClaim, Urgency};
use hyperform_protocol::{Message, Question, ToolResultPart, ToolStatus, TurnRecord};
use hyperform_tools::RankRequest;
use hyperform_verify::CheckSpec;

use super::context::UseContext;
use super::seams::{AttentionItem, ClearAdvice, Finding, ResultText, RouteAdvice, RouteTask};
use crate::batch::ToolCall;

/// Where the engine consults uses, in turn order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seam {
    /// The user's message arrived, before the agent run starts.
    TurnStart,
    /// Context pressure is about to clear old tool results.
    Pressure,
    /// A request is about to reach the model provider; a use may flag
    /// likely credentials in tool results it has not seen yet.
    BeforeRequest,
    /// The policy allowed a tool call; a use may only ask for approval.
    ToolGate,
    /// A tool call finished; a use may prepend notes to its result.
    AfterCall,
    /// The main agent stopped and verification is done.
    Stop,
    /// `AskUserQuestion` is about to reach the user; a use may point the
    /// model at an earlier answer instead.
    AskUser,
    /// A task starts: the main agent's new task, or a subagent whose model
    /// is `inherit`; a use may route it within what the task allows.
    Route,
    /// The main agent stopped with changes no check backs; a use may say
    /// its final message claims success anyway.
    Completion,
    /// Verification is about to run its automatic checks; a use may name
    /// checks the changes cannot affect.
    CheckSelect,
    /// A tool is about to cut a long result; a use may say which parts
    /// matter for the task.
    Relevance,
    /// A main-agent turn ended; runs in the background.
    TurnEnd,
    /// Something needs the user or tells them something; a use may rate
    /// how urgent it is.
    Attention,
}

/// Each method's advice is applied only in `on` mode; in `shadow` mode it
/// runs in the background and only its trace records remain.
#[async_trait]
pub(crate) trait DecisionUse: Send + Sync + std::fmt::Debug {
    fn feature(&self) -> FeatureId;

    fn seams(&self) -> &'static [Seam];

    /// Reminders for the opening message of the turn.
    async fn turn_start(&self, _cx: &UseContext, _text: &str) -> Vec<String> {
        Vec::new()
    }

    /// Changes to the planned clears; `planned` is oldest first.
    async fn pressure(
        &self,
        _cx: &UseContext,
        _working: &[Message],
        _planned: &[ClearTarget],
    ) -> ClearAdvice {
        ClearAdvice::default()
    }

    /// Values in new tool `results` that look like credentials.
    async fn before_request(&self, _cx: &UseContext, _results: &[ResultText]) -> Vec<Finding> {
        Vec::new()
    }

    /// A reason to ask the user before an allowed call runs.
    async fn tool_gate(&self, _cx: &UseContext, _call: &ToolCall) -> Option<String> {
        None
    }

    /// Notes to prepend to the call's result.
    async fn after_call(
        &self,
        _cx: &UseContext,
        _call: &ToolCall,
        _status: ToolStatus,
        _output: &[ToolResultPart],
    ) -> Vec<String> {
        Vec::new()
    }

    /// A reminder that sends the agent back to work.
    async fn stop(&self, _cx: &UseContext, _changed: &[PathBuf]) -> Option<String> {
        None
    }

    /// A tool result for the model instead of asking the user `questions`.
    async fn ask_user(&self, _cx: &UseContext, _questions: &[Question]) -> Option<String> {
        None
    }

    /// The effort or model for a task that starts.
    async fn route(&self, _cx: &UseContext, _task: &RouteTask) -> Option<RouteAdvice> {
        None
    }

    /// What the final message claims while no check backs the changes.
    async fn completion(&self, _cx: &UseContext, _changed: &[PathBuf]) -> Option<UncheckedClaim> {
        None
    }

    /// Ids of `checks` the `changed` paths cannot affect.
    async fn check_select(
        &self,
        _cx: &UseContext,
        _checks: &[CheckSpec],
        _changed: &[PathBuf],
    ) -> Vec<String> {
        Vec::new()
    }

    /// Per item of `request`, whether it matters for the task.
    async fn relevance(
        &self,
        _cx: &UseContext,
        _request: &RankRequest,
    ) -> Option<Vec<Option<bool>>> {
        None
    }

    /// How the turn that just ended went.
    async fn turn_end(&self, _cx: &UseContext, _turn: &TurnRecord) -> Option<TurnTone> {
        None
    }

    /// Per item, how urgent it is for the user.
    async fn attention(&self, _cx: &UseContext, _items: &[AttentionItem]) -> Vec<Option<Urgency>> {
        Vec::new()
    }
}

/// Every shipped use, in the order its advice is applied. Add a use here
/// once its file in `uses/` implements `DecisionUse` and its registry entry
/// in `hyperform-config` is `.available()`.
pub(crate) static USES: &[&dyn DecisionUse] = &[
    &super::uses::compaction::COMPACTION,
    &super::uses::task_view::TASK_VIEW,
    &super::uses::session_context::SESSION_CONTEXT,
    &super::uses::routing::ROUTING,
    &super::uses::loop_guard::LOOP_GUARD,
    &super::uses::completion_check::COMPLETION_CHECK,
    &super::uses::risk::RISK,
    &super::uses::output_trim::OUTPUT_TRIM,
    &super::uses::search_rank::SEARCH_RANK,
    &super::uses::prefetch::PREFETCH,
    &super::uses::plan_suggest::PLAN_SUGGEST,
    &super::uses::user_signal::USER_SIGNAL,
    &super::uses::hints::HINTS,
    &super::uses::memory_suggest::MEMORY_SUGGEST,
    &super::uses::question_check::QUESTION_CHECK,
    &super::uses::review_suggest::REVIEW_SUGGEST,
    &super::uses::pet_mood::PET_MOOD,
    &super::uses::inbox_priority::INBOX_PRIORITY,
    &super::uses::check_select::CHECK_SELECT,
    &super::uses::custom_rules::CUSTOM_RULES,
    &super::uses::secret_screen::SECRET_SCREEN,
];

#[cfg(test)]
mod tests {
    use hyperform_config::FeatureGroup;

    use super::*;

    #[test]
    fn uses_are_the_available_features_in_registry_order() {
        let used: Vec<FeatureId> = USES.iter().map(|u| u.feature()).collect();
        let available = FeatureId::ALL
            .into_iter()
            .filter(|id| id.spec().available && id.spec().group == FeatureGroup::Decisions);
        assert_eq!(used, available.collect::<Vec<_>>());
    }
}
