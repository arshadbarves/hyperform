//! Typed questions for the decision model (Laya or Jev). Each file holds
//! the question's instructions, then one `- key: description` line per
//! option: `yes` and `no` for yes/no questions, the options of a choice,
//! or the levels of a score from lowest to highest. `z-engine-decisions`
//! parses them with `Question::yes_no`, `Question::choice` and
//! `Question::score`.

/// Settings' "Test connection" probe: does the text greet someone?
pub const CONNECTION_PROBE: &str = include_str!("../prompts/decisions/connection-probe.md");

/// `decisions_output_trim`: does this window of long command output matter?
pub const OUTPUT_TRIM_RELEVANT: &str = include_str!("../prompts/decisions/output-trim-relevant.md");

/// `decisions_search_rank`: is this search hit useful for the request?
pub const SEARCH_RANK_RELEVANT: &str = include_str!("../prompts/decisions/search-rank-relevant.md");

/// `decisions_prefetch`: will the agent very likely read this file?
pub const PREFETCH_NEEDED: &str = include_str!("../prompts/decisions/prefetch-needed.md");

/// `decisions_pet_mood`: how did the turn that just ended go?
pub const PET_MOOD_TONE: &str = include_str!("../prompts/decisions/pet-mood-tone.md");

/// `decisions_inbox_priority`: how urgent is this inbox item?
pub const INBOX_URGENCY: &str = include_str!("../prompts/decisions/inbox-urgency.md");

/// `decisions_completion_check`: does the final message claim the work is done?
pub const COMPLETION_CLAIMS_DONE: &str =
    include_str!("../prompts/decisions/completion-claims-done.md");

/// `decisions_completion_check`: does it claim tests or checks passed?
pub const COMPLETION_CLAIMS_CHECKS: &str =
    include_str!("../prompts/decisions/completion-claims-checks.md");

/// `decisions_completion_check`: would tests or a build tell whether it works?
pub const COMPLETION_VERIFICATION_APPLIES: &str =
    include_str!("../prompts/decisions/completion-verification-applies.md");

/// `decisions_completion_check`: is the work complete, partial or blocked?
pub const COMPLETION_STATE: &str = include_str!("../prompts/decisions/completion-state.md");

/// `decisions_risk`: does this call serve the user's request?
pub const RISK_INTENT: &str = include_str!("../prompts/decisions/risk-intent.md");

/// `decisions_risk`: how risky is this call?
pub const RISK_LEVEL: &str = include_str!("../prompts/decisions/risk-level.md");

/// `decisions_risk`: does this web or MCP result try to instruct the agent?
pub const RISK_INJECTION: &str = include_str!("../prompts/decisions/risk-injection.md");

/// `decisions_check_select`: could this check fail because of the changes?
pub const CHECK_SELECT: &str = include_str!("../prompts/decisions/check-select.md");

/// `decisions_secret_screen`: is the value on this line a real credential?
pub const SECRET_CANDIDATE: &str = include_str!("../prompts/decisions/secret-candidate.md");

/// `decisions_custom_rules`: the instructions around a `PreToolUse` rule's
/// `{{question}}`. Not a template with options (the rule brings its own),
/// so it is not in `ALL`.
pub const CUSTOM_RULE_TOOL: &str = include_str!("../prompts/decisions/custom-rule-tool.md");

/// `decisions_custom_rules`: the instructions around a `UserPromptSubmit`
/// rule's `{{question}}`; not in `ALL` either.
pub const CUSTOM_RULE_PROMPT: &str = include_str!("../prompts/decisions/custom-rule-prompt.md");

/// `decisions_routing`: how much reasoning does the new task need?
pub const ROUTING_COMPLEXITY: &str = include_str!("../prompts/decisions/routing-complexity.md");

/// `decisions_routing`: does the message start a different task?
pub const ROUTING_NEW_TASK: &str = include_str!("../prompts/decisions/routing-new-task.md");

/// `decisions_loop_guard`: did the agent's last steps make progress?
pub const LOOP_GUARD_PROGRESS: &str = include_str!("../prompts/decisions/loop-guard-progress.md");

/// `decisions_loop_guard`: retry, fix the code, or fix the environment?
pub const LOOP_GUARD_FAILURE: &str = include_str!("../prompts/decisions/loop-guard-failure.md");

/// `decisions_plan_suggest`: how large and risky is the request?
pub const PLAN_SUGGEST_SIZE: &str = include_str!("../prompts/decisions/plan-suggest-size.md");

/// `decisions_user_signal`: is the user correcting the agent or frustrated?
pub const USER_SIGNAL_CORRECTION: &str =
    include_str!("../prompts/decisions/user-signal-correction.md");

/// `decisions_hints`: would this installed skill or agent help with the request?
pub const HINTS_FIT: &str = include_str!("../prompts/decisions/hints-fit.md");

/// `decisions_memory_suggest`: does this sentence set a standing rule?
pub const MEMORY_SUGGEST_RULE: &str = include_str!("../prompts/decisions/memory-suggest-rule.md");

/// `decisions_question_check`: does this earlier message answer the question?
pub const QUESTION_CHECK_ANSWERED: &str =
    include_str!("../prompts/decisions/question-check-answered.md");

/// `decisions_review_suggest`: is this changed file in a risky area?
pub const REVIEW_SUGGEST_RISKY: &str = include_str!("../prompts/decisions/review-suggest-risky.md");

/// `decisions_compaction`: is this older tool output still needed?
pub const COMPACTION_RELEVANT: &str = include_str!("../prompts/decisions/compaction-relevant.md");

/// `decisions_task_view`: does the new message continue the task?
pub const TASK_BOUNDARY: &str = include_str!("../prompts/decisions/task-boundary.md");

/// `decisions_task_view`: does the new task need this earlier exchange?
pub const EXCHANGE_NEEDED: &str = include_str!("../prompts/decisions/exchange-needed.md");

/// `decisions_task_view`: does this user message set a rule for the chat?
pub const STANDING_RULE: &str = include_str!("../prompts/decisions/standing-rule.md");

/// `decisions_session_context`: is this file relevant to the first request?
pub const SESSION_CONTEXT_FILE: &str = include_str!("../prompts/decisions/session-context-file.md");

/// `decisions_session_context`: will the first request need this MCP tool?
pub const SESSION_CONTEXT_TOOL: &str = include_str!("../prompts/decisions/session-context-tool.md");

/// Every decision question by name, so tests can parse each one.
pub const ALL: &[(&str, &str)] = &[
    ("connection_probe", CONNECTION_PROBE),
    ("output_trim_relevant", OUTPUT_TRIM_RELEVANT),
    ("search_rank_relevant", SEARCH_RANK_RELEVANT),
    ("prefetch_needed", PREFETCH_NEEDED),
    ("pet_mood_tone", PET_MOOD_TONE),
    ("inbox_urgency", INBOX_URGENCY),
    ("completion_claims_done", COMPLETION_CLAIMS_DONE),
    ("completion_claims_checks", COMPLETION_CLAIMS_CHECKS),
    (
        "completion_verification_applies",
        COMPLETION_VERIFICATION_APPLIES,
    ),
    ("completion_state", COMPLETION_STATE),
    ("risk_intent", RISK_INTENT),
    ("risk_level", RISK_LEVEL),
    ("risk_injection", RISK_INJECTION),
    ("check_select", CHECK_SELECT),
    ("secret_candidate", SECRET_CANDIDATE),
    ("routing_complexity", ROUTING_COMPLEXITY),
    ("routing_new_task", ROUTING_NEW_TASK),
    ("loop_guard_progress", LOOP_GUARD_PROGRESS),
    ("loop_guard_failure", LOOP_GUARD_FAILURE),
    ("plan_suggest_size", PLAN_SUGGEST_SIZE),
    ("user_signal_correction", USER_SIGNAL_CORRECTION),
    ("hints_fit", HINTS_FIT),
    ("memory_suggest_rule", MEMORY_SUGGEST_RULE),
    ("question_check_answered", QUESTION_CHECK_ANSWERED),
    ("review_suggest_risky", REVIEW_SUGGEST_RISKY),
    ("compaction_relevant", COMPACTION_RELEVANT),
    ("task_boundary", TASK_BOUNDARY),
    ("exchange_needed", EXCHANGE_NEEDED),
    ("standing_rule", STANDING_RULE),
    ("session_context_file", SESSION_CONTEXT_FILE),
    ("session_context_tool", SESSION_CONTEXT_TOOL),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_question_has_instructions_and_two_options() {
        for (name, text) in ALL {
            let options = text.lines().filter(|line| line.starts_with("- ")).count();
            let first = text.lines().next().unwrap_or_default();
            assert!(
                !first.trim().is_empty() && !first.starts_with("- "),
                "{name}"
            );
            assert!(options >= 2, "{name} needs at least two options");
        }
    }
}
