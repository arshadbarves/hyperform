//! Built-in tool names (Claude Code-compatible), in registry order.

pub const READ: &str = "Read";
pub const WRITE: &str = "Write";
pub const EDIT: &str = "Edit";
pub const MULTI_EDIT: &str = "MultiEdit";
pub const NOTEBOOK_EDIT: &str = "NotebookEdit";
pub const GLOB: &str = "Glob";
pub const GREP: &str = "Grep";
pub const BASH: &str = "Bash";
pub const JOB_OUTPUT: &str = "JobOutput";
pub const JOB_KILL: &str = "JobKill";
pub const WEB_FETCH: &str = "WebFetch";
pub const WEB_SEARCH: &str = "WebSearch";
pub const TODO_WRITE: &str = "TodoWrite";
pub const ASK_USER_QUESTION: &str = "AskUserQuestion";
pub const EXIT_PLAN_MODE: &str = "ExitPlanMode";
pub const TASK_CREATE: &str = "TaskCreate";
pub const TASK_UPDATE: &str = "TaskUpdate";
pub const TASK_GET: &str = "TaskGet";
pub const TASK_LIST: &str = "TaskList";
pub const PLAN_RECORD: &str = "PlanRecord";
pub const SKILL: &str = "Skill";
pub const AGENT: &str = "Agent";
pub const APPLY_AGENT_CHANGES: &str = "ApplyAgentChanges";
pub const VERIFY: &str = "Verify";
pub const LSP: &str = "LSP";
pub const LIST_MCP_RESOURCES: &str = "ListMcpResources";
pub const READ_MCP_RESOURCE: &str = "ReadMcpResource";
/// Registered by the engine only while MCP tools are deferred; not in
/// [`ALL`].
pub const LOAD_MCP_TOOLS: &str = "LoadMcpTools";

/// The work-plan tools: registered with the built-ins, offered only while
/// `work_plans` is on.
pub const WORK: [&str; 5] = [TASK_CREATE, TASK_UPDATE, TASK_GET, TASK_LIST, PLAN_RECORD];

/// Every built-in tool, in the order `ToolRegistry::builtin` registers them.
pub const ALL: [&str; 27] = [
    READ,
    WRITE,
    EDIT,
    MULTI_EDIT,
    NOTEBOOK_EDIT,
    GLOB,
    GREP,
    BASH,
    JOB_OUTPUT,
    JOB_KILL,
    WEB_FETCH,
    WEB_SEARCH,
    TODO_WRITE,
    ASK_USER_QUESTION,
    EXIT_PLAN_MODE,
    TASK_CREATE,
    TASK_UPDATE,
    TASK_GET,
    TASK_LIST,
    PLAN_RECORD,
    SKILL,
    AGENT,
    APPLY_AGENT_CHANGES,
    VERIFY,
    LSP,
    LIST_MCP_RESOURCES,
    READ_MCP_RESOURCE,
];
