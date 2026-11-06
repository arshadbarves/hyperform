//! Model-facing tool descriptions, one file per tool.

/// Reads text (numbered like `cat -n`), images, PDFs, and notebooks.
pub const READ: &str = include_str!("../prompts/tools/read.md");

/// Creates or fully replaces a file that was read first.
pub const WRITE: &str = include_str!("../prompts/tools/write.md");

/// One exact string replacement, with the fallback ladder.
pub const EDIT: &str = include_str!("../prompts/tools/edit.md");

/// Several replacements in one file, all-or-nothing.
pub const MULTI_EDIT: &str = include_str!("../prompts/tools/multi_edit.md");

/// Replaces, inserts, or deletes one notebook cell.
pub const NOTEBOOK_EDIT: &str = include_str!("../prompts/tools/notebook_edit.md");

/// Finds files by glob pattern.
pub const GLOB: &str = include_str!("../prompts/tools/glob.md");

/// ripgrep content search.
pub const GREP: &str = include_str!("../prompts/tools/grep.md");

/// Shell commands, background jobs, and git safety.
pub const BASH: &str = include_str!("../prompts/tools/bash.md");

/// New output of a background job.
pub const JOB_OUTPUT: &str = include_str!("../prompts/tools/job_output.md");

/// Stops a background job.
pub const JOB_KILL: &str = include_str!("../prompts/tools/job_kill.md");

/// Fetches a page and answers a prompt about it.
pub const WEB_FETCH: &str = include_str!("../prompts/tools/web_fetch.md");

/// Web search results.
pub const WEB_SEARCH: &str = include_str!("../prompts/tools/web_search.md");

/// The agent's todo list.
pub const TODO_WRITE: &str = include_str!("../prompts/tools/todo_write.md");

/// Structured multiple-choice questions for the user.
pub const ASK_USER_QUESTION: &str = include_str!("../prompts/tools/ask_user_question.md");

/// Submits a plan for review in plan mode.
pub const EXIT_PLAN_MODE: &str = include_str!("../prompts/tools/exit_plan_mode.md");

/// `work_plans`: creates a task in the attached plan (starting one).
pub const TASK_CREATE: &str = include_str!("../prompts/tools/task_create.md");

/// `work_plans`: changes a task's status, fields, dependencies or criteria.
pub const TASK_UPDATE: &str = include_str!("../prompts/tools/task_update.md");

/// `work_plans`: one task's packet.
pub const TASK_GET: &str = include_str!("../prompts/tools/task_get.md");

/// `work_plans`: the plan's tasks by state, with a health line.
pub const TASK_LIST: &str = include_str!("../prompts/tools/task_list.md");

/// `work_plans`: records a decision, discovery, experiment, evidence note,
/// or a proposed requirement or invariant.
pub const PLAN_RECORD: &str = include_str!("../prompts/tools/plan_record.md");

/// Loads a skill's instructions.
pub const SKILL: &str = include_str!("../prompts/tools/skill.md");

/// Launches a subagent. Template: `{{agents}}` is replaced with one
/// `- name: description` line per available agent type.
pub const AGENT: &str = include_str!("../prompts/tools/agent.md");

/// Merges a worktree agent's changes into the tree.
pub const APPLY_AGENT_CHANGES: &str = include_str!("../prompts/tools/apply_agent_changes.md");

/// Lists and runs the project's checks as evidence.
pub const VERIFY: &str = include_str!("../prompts/tools/verify.md");

/// Language-server queries.
pub const LSP: &str = include_str!("../prompts/tools/lsp.md");

/// Resources exposed by MCP servers.
pub const LIST_MCP_RESOURCES: &str = include_str!("../prompts/tools/list_mcp_resources.md");

/// Reads one MCP resource.
pub const READ_MCP_RESOURCE: &str = include_str!("../prompts/tools/read_mcp_resource.md");

/// Loads deferred MCP tool definitions. Registered by the engine only
/// while MCP tools are deferred, so it is not part of [`BUILTIN`].
pub const LOAD_MCP_TOOLS: &str = include_str!("../prompts/tools/load_mcp_tools.md");

/// Every built-in tool as `(tool name, description)`, in registry order.
pub const BUILTIN: &[(&str, &str)] = &[
    ("Read", READ),
    ("Write", WRITE),
    ("Edit", EDIT),
    ("MultiEdit", MULTI_EDIT),
    ("NotebookEdit", NOTEBOOK_EDIT),
    ("Glob", GLOB),
    ("Grep", GREP),
    ("Bash", BASH),
    ("JobOutput", JOB_OUTPUT),
    ("JobKill", JOB_KILL),
    ("WebFetch", WEB_FETCH),
    ("WebSearch", WEB_SEARCH),
    ("TodoWrite", TODO_WRITE),
    ("AskUserQuestion", ASK_USER_QUESTION),
    ("ExitPlanMode", EXIT_PLAN_MODE),
    ("TaskCreate", TASK_CREATE),
    ("TaskUpdate", TASK_UPDATE),
    ("TaskGet", TASK_GET),
    ("TaskList", TASK_LIST),
    ("PlanRecord", PLAN_RECORD),
    ("Skill", SKILL),
    ("Agent", AGENT),
    ("ApplyAgentChanges", APPLY_AGENT_CHANGES),
    ("Verify", VERIFY),
    ("LSP", LSP),
    ("ListMcpResources", LIST_MCP_RESOURCES),
    ("ReadMcpResource", READ_MCP_RESOURCE),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptions_are_not_empty_and_names_are_unique() {
        for (index, (name, text)) in BUILTIN.iter().enumerate() {
            assert!(!text.trim().is_empty(), "{name}: empty description");
            assert!(
                BUILTIN[index + 1..].iter().all(|(other, _)| other != name),
                "duplicate tool `{name}`"
            );
        }
    }

    #[test]
    fn only_the_agent_template_has_a_placeholder() {
        assert_eq!(AGENT.matches("{{agents}}").count(), 1);
        assert!(!AGENT.replace("{{agents}}", "").contains("{{"));
        for (name, text) in BUILTIN.iter().filter(|(name, _)| *name != "Agent") {
            assert!(!text.contains("{{"), "{name}: unexpected placeholder");
        }
        assert!(!LOAD_MCP_TOOLS.trim().is_empty() && !LOAD_MCP_TOOLS.contains("{{"));
    }
}
