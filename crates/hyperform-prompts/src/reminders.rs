//! System reminders injected between rounds, and the environment template.
//!
//! Templates use `{{name}}` placeholders; a line holding an optional
//! `{{name?}}` placeholder is dropped when its value is empty.
//! `hyperform-context` renders them and wraps reminders in
//! `<system-reminder>` tags.

/// Environment block of the system prompt.
pub const ENVIRONMENT: &str = include_str!("../prompts/reminders/environment.md");

/// Git snapshot embedded in [`ENVIRONMENT`] for git repositories.
pub const GIT_SNAPSHOT: &str = include_str!("../prompts/reminders/git-snapshot.md");

/// Preamble placed before the project and user instruction files.
pub const INSTRUCTIONS_PREAMBLE: &str =
    include_str!("../prompts/reminders/instructions-preamble.md");

/// Skills section of the system prompt; `{{skills}}` holds one line per skill.
pub const SKILLS_LISTING: &str = include_str!("../prompts/reminders/skills-listing.md");

/// The todo list is empty; suggest `TodoWrite` for multi-step work.
pub const TODO_NUDGE: &str = include_str!("../prompts/reminders/todo-nudge.md");

/// The current todo list.
pub const TODO_STATE: &str = include_str!("../prompts/reminders/todo-state.md");

/// Plan mode rules: read-only research, then `ExitPlanMode`.
pub const PLAN_MODE_ACTIVE: &str = include_str!("../prompts/reminders/plan-mode-active.md");

/// The user approved the plan; implementation may start.
pub const PLAN_APPROVED: &str = include_str!("../prompts/reminders/plan-approved.md");

/// Files changed on disk since the model last read them.
pub const FILES_CHANGED_EXTERNALLY: &str =
    include_str!("../prompts/reminders/files-changed-externally.md");

/// Instruction files found in directories the model just accessed.
pub const NESTED_INSTRUCTIONS: &str = include_str!("../prompts/reminders/nested-instructions.md");

/// A background shell or agent job finished.
pub const JOB_FINISHED: &str = include_str!("../prompts/reminders/job-finished.md");

/// Messages the user sent while the model was working.
pub const STEERING: &str = include_str!("../prompts/reminders/steering.md");

/// Changed files are unverified under `auto` or `strict` verification.
pub const VERIFICATION_REQUIRED: &str =
    include_str!("../prompts/reminders/verification-required.md");

/// Checks run automatically at the stop boundary failed.
pub const AUTO_CHECK_FAILED: &str = include_str!("../prompts/reminders/auto-check-failed.md");

/// MCP tools are deferred; `{{tools}}` holds one `- name: summary` line per
/// tool, loadable with `LoadMcpTools`.
pub const MCP_TOOLS_DEFERRED: &str = include_str!("../prompts/reminders/mcp-tools-deferred.md");

/// Error diagnostics of a file the model just wrote; `{{file}}` is its
/// root-relative path, `{{diagnostics}}` one line per error.
pub const LSP_ERRORS: &str = include_str!("../prompts/reminders/lsp-errors.md");

/// The user mentioned `@agent-<name>`; `{{agent}}` is the agent type.
pub const AGENT_MENTION: &str = include_str!("../prompts/reminders/agent-mention.md");

/// The user interrupted the previous response.
pub const INTERRUPTED: &str = include_str!("../prompts/reminders/interrupted.md");

/// User message that replaces summarized history after compaction.
pub const COMPACTION_SUMMARY: &str = include_str!("../prompts/reminders/compaction-summary.md");

/// `decisions_risk`: a web or MCP result may hold prompt injection;
/// `{{tool}}` names the tool.
pub const INJECTION_WARNING: &str = include_str!("../prompts/reminders/injection-warning.md");

/// `decisions_custom_rules` with `action = "ask"` on a prompt; `{{question}}`
/// is the rule, `{{answer}}` the option that fired it.
pub const CUSTOM_RULE_ASK: &str = include_str!("../prompts/reminders/custom-rule-ask.md");

/// `decisions_custom_rules` with `action = "remind"`; `{{tool?}}` names the
/// call it was asked about.
pub const CUSTOM_RULE_REMIND: &str = include_str!("../prompts/reminders/custom-rule-remind.md");

/// `decisions_secret_screen`: what replaces a credential the user kept from
/// the model; `{{kind}}` names the detector.
pub const SECRET_MASKED: &str = include_str!("../prompts/reminders/secret-masked.md");

/// `decisions_prefetch`: a file read ahead of the request; `{{path}}` is its
/// root-relative path, `{{body}}` its content.
pub const PREFETCHED_FILE: &str = include_str!("../prompts/reminders/prefetched-file.md");

/// `decisions_loop_guard`: the same call with the same result again;
/// `{{tool}}` names it, `{{count}}` the repeats.
pub const LOOP_REPEAT: &str = include_str!("../prompts/reminders/loop-repeat.md");

/// `decisions_loop_guard`: the model judged the last `{{count}}` steps
/// as making no progress.
pub const LOOP_NO_PROGRESS: &str = include_str!("../prompts/reminders/loop-no-progress.md");

/// `decisions_loop_guard`: the same failure after an edit, from the code;
/// `{{tool}}` and `{{count}}` as in [`LOOP_REPEAT`].
pub const LOOP_FAILURE_CODE: &str = include_str!("../prompts/reminders/loop-failure-code.md");

/// `decisions_loop_guard`: the same failure, from the environment.
pub const LOOP_FAILURE_ENVIRONMENT: &str =
    include_str!("../prompts/reminders/loop-failure-environment.md");

/// `decisions_loop_guard`: the same failure, transient; retry once.
pub const LOOP_FAILURE_RETRY: &str = include_str!("../prompts/reminders/loop-failure-retry.md");

/// `decisions_user_signal`: the user seems to be correcting the agent.
pub const CORRECTION_SIGNAL: &str = include_str!("../prompts/reminders/correction-signal.md");

/// `decisions_hints`: installed skills or agents that look relevant;
/// `{{hints}}` holds one `- kind `name`: description` line each.
pub const SKILL_HINTS: &str = include_str!("../prompts/reminders/skill-hints.md");

/// `decisions_question_check`: the tool result instead of asking;
/// `{{answers}}` holds one line per question the chat already answered.
pub const QUESTION_ANSWERED: &str = include_str!("../prompts/reminders/question-answered.md");

/// `decisions_task_view`: the earlier exchanges set aside for the current
/// task; `{{exchanges}}` holds one `Turn <n>: ...` line each.
pub const TASK_VIEW_INDEX: &str = include_str!("../prompts/reminders/task-view-index.md");

/// `work_plans`: the turn-start pin while a plan is attached; `{{goal}}`,
/// `{{status}}` and `{{revision}}` describe the plan, `{{current?}}` and
/// `{{next?}}` are `id: title` of the agent's task and the next ready one.
pub const WORK_PIN: &str = include_str!("../prompts/reminders/work-pin.md");

/// `work_plans`: a full task packet; `{{task}}` is the task id,
/// `{{packet}}` the packet rendered from [`crate::work::PACKET`].
pub const WORK_PACKET: &str = include_str!("../prompts/reminders/work-packet.md");

/// `work_plans`: reconciliation findings moved the plan to replanning;
/// `{{findings}}` holds one `- ...` line per finding.
pub const WORK_RECONCILE: &str = include_str!("../prompts/reminders/work-reconcile.md");

/// `work_plans`: a task's helper ended, was interrupted or its change came
/// back; `{{agent}}`, `{{task}}`, `{{event}}` and `{{status}}` say what
/// happened, `{{summary?}}` is the report's summary, `{{ready?}}`,
/// `{{parallel?}}` and `{{held?}}` hold one `- id: title` line each.
pub const WORK_COORDINATE: &str = include_str!("../prompts/reminders/work-coordinate.md");

/// `work_plans`: the user added an audit finding as task `{{task}}`;
/// `{{finding}}` is its text, `{{suggestion?}}` the suggested change,
/// `{{refs?}}` what it is about.
pub const WORK_FINDING_ADDED: &str = include_str!("../prompts/reminders/work-finding-added.md");

/// `work_plans`: the first turn after this chat opened with its plan (an
/// app restart too) or attached it; `{{goal}}`, `{{status}}` and
/// `{{summary}}` (what resuming found) describe the plan, `{{completed?}}`,
/// `{{current?}}`, `{{issues?}}` and `{{findings?}}` hold one `- ...` line
/// each, `{{next?}}` is the next ready task.
pub const WORK_RESUME: &str = include_str!("../prompts/reminders/work-resume.md");
