//! The contract every built-in tool keeps: object schemas with the
//! documented Claude Code field names, rendered descriptions, total
//! `action`/`title`, and the declared read-only and concurrency flags.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use z_engine_tools::{AgentCard, ToolCtx, ToolRegistry, names};

/// `(tool, properties, required)` as documented.
const FIELDS: &[(&str, &[&str], &[&str])] = &[
    (
        "Read",
        &["file_path", "offset", "limit", "pages"],
        &["file_path"],
    ),
    (
        "Write",
        &["file_path", "content"],
        &["file_path", "content"],
    ),
    (
        "Edit",
        &["file_path", "old_string", "new_string", "replace_all"],
        &["file_path", "old_string", "new_string"],
    ),
    (
        "MultiEdit",
        &["file_path", "edits"],
        &["file_path", "edits"],
    ),
    (
        "NotebookEdit",
        &[
            "notebook_path",
            "cell_id",
            "new_source",
            "cell_type",
            "edit_mode",
        ],
        &["notebook_path", "new_source"],
    ),
    ("Glob", &["pattern", "path"], &["pattern"]),
    (
        "Grep",
        &[
            "pattern",
            "path",
            "glob",
            "type",
            "output_mode",
            "-i",
            "-n",
            "-A",
            "-B",
            "-C",
            "multiline",
            "head_limit",
            "offset",
        ],
        &["pattern"],
    ),
    (
        "Bash",
        &["command", "description", "timeout", "run_in_background"],
        &["command"],
    ),
    ("JobOutput", &["job_id", "filter", "wait_ms"], &["job_id"]),
    ("JobKill", &["job_id"], &["job_id"]),
    ("WebFetch", &["url", "prompt"], &["url", "prompt"]),
    (
        "WebSearch",
        &["query", "allowed_domains", "blocked_domains"],
        &["query"],
    ),
    ("TodoWrite", &["todos"], &["todos"]),
    ("AskUserQuestion", &["questions"], &["questions"]),
    ("ExitPlanMode", &["plan"], &["plan"]),
    (
        "TaskCreate",
        &[
            "subject",
            "description",
            "activeForm",
            "parentId",
            "blockedBy",
            "acceptance",
            "requirements",
            "kind",
            "priority",
            "scope",
        ],
        &["subject", "description"],
    ),
    (
        "TaskUpdate",
        &[
            "taskId",
            "status",
            "reason",
            "subject",
            "description",
            "activeForm",
            "result",
            "acceptance",
            "addBlockedBy",
            "removeBlockedBy",
            "addBlocks",
            "metCriteria",
            "kind",
            "priority",
            "scope",
            "report",
        ],
        &["taskId"],
    ),
    ("TaskGet", &["taskId"], &["taskId"]),
    ("TaskList", &[], &[]),
    (
        "PlanRecord",
        &[
            "kind",
            "text",
            "title",
            "alternatives",
            "reason",
            "source",
            "method",
            "experimentId",
            "outcome",
            "result",
            "conclusion",
            "taskIds",
            "files",
            "criterionId",
        ],
        &["kind", "text"],
    ),
    ("Skill", &["skill"], &["skill"]),
    (
        "Agent",
        &[
            "description",
            "prompt",
            "subagent_type",
            "run_in_background",
            "resume",
            "isolation",
        ],
        &["description", "prompt", "subagent_type"],
    ),
    ("ApplyAgentChanges", &["agent_id"], &["agent_id"]),
    ("Verify", &["action", "check"], &["action"]),
    (
        "LSP",
        &[
            "operation",
            "file_path",
            "line",
            "character",
            "query",
            "new_name",
        ],
        &["operation"],
    ),
    ("ListMcpResources", &["server"], &[]),
    ("ReadMcpResource", &["server", "uri"], &["server", "uri"]),
];

/// Tools that only observe state for any input.
const READ_ONLY: &[&str] = &[
    "Read",
    "Glob",
    "Grep",
    "JobOutput",
    "JobKill",
    "WebFetch",
    "WebSearch",
    "TodoWrite",
    "AskUserQuestion",
    "ExitPlanMode",
    "TaskCreate",
    "TaskUpdate",
    "TaskGet",
    "TaskList",
    "PlanRecord",
    "Skill",
    "LSP",
    "ListMcpResources",
    "ReadMcpResource",
];

/// Read-only tools that must still run alone.
const SERIAL_READ_ONLY: &[&str] = &[
    "TodoWrite",
    "AskUserQuestion",
    "ExitPlanMode",
    "TaskCreate",
    "TaskUpdate",
    "PlanRecord",
];

fn registry() -> ToolRegistry {
    ToolRegistry::builtin(vec![AgentCard {
        name: "explore".into(),
        description: "Fast search.".into(),
        tools: "Read".into(),
    }])
}

fn keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn every_schema_is_an_object_with_the_documented_fields() {
    let registry = registry();
    assert_eq!(FIELDS.len(), names::ALL.len());
    for (name, properties, required) in FIELDS {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("{name} is not registered"));
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object", "{name}");
        let expected: BTreeSet<String> = properties.iter().map(|p| p.to_string()).collect();
        assert_eq!(keys(&schema["properties"]), expected, "{name} properties");
        let listed: Vec<&str> = schema["required"]
            .as_array()
            .unwrap_or_else(|| panic!("{name}: required must be an array"))
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(listed, required.to_vec(), "{name} required");
        for property in schema["properties"].as_object().into_iter().flatten() {
            assert!(
                property.1.get("type").is_some(),
                "{name}.{}: no type",
                property.0
            );
        }
    }
}

#[test]
fn descriptions_are_rendered_and_non_empty() {
    for (name, description, _) in registry().specs() {
        assert!(
            description.trim().len() > 80,
            "{name}: description too short"
        );
        assert!(
            !description.contains("{{"),
            "{name}: unrendered placeholder"
        );
    }
    let agent = registry().get("Agent").unwrap().description();
    assert!(
        agent.contains("- explore: Fast search. (Tools: Read)"),
        "{agent}"
    );
}

#[test]
fn action_and_title_are_total_and_flags_match_the_contract() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ToolCtx::for_tests(dir.path());
    for tool in registry().iter() {
        for input in [json!({}), json!(null), json!({"file_path": 7})] {
            let action = format!("{:?}", tool.action(&input, &ctx));
            assert!(!action.is_empty(), "{}: no action", tool.name());
            assert!(
                !tool.title(&input, &ctx).is_empty(),
                "{}: empty title",
                tool.name()
            );
        }
        let input = json!({});
        let read_only = READ_ONLY.contains(&tool.name()) || tool.name() == "Verify";
        assert_eq!(
            tool.is_read_only(&input),
            read_only,
            "{} read-only",
            tool.name()
        );
        let concurrent =
            (read_only && !SERIAL_READ_ONLY.contains(&tool.name())) || tool.name() == "Agent";
        assert_eq!(
            tool.is_concurrency_safe(&input),
            concurrent,
            "{} concurrency",
            tool.name()
        );
    }
}
