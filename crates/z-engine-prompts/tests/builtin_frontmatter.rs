//! Built-in agents and commands start with YAML frontmatter that the config
//! crate can parse, and every registry entry is exactly its markdown file.

use serde_yaml_ng::Mapping;
use z_engine_prompts::{agents, commands};

const AGENT_KEYS: &[&str] = &[
    "name",
    "description",
    "tools",
    "model",
    "permissionMode",
    "isolation",
    "maxTurns",
    "color",
];
const COMMAND_KEYS: &[&str] = &["description", "argument-hint", "allowed-tools"];
const MODELS: &[&str] = &["inherit", "fast", "main", "review"];
const PERMISSION_MODES: &[&str] = &["default", "acceptEdits", "plan", "bypass"];
const ISOLATIONS: &[&str] = &["shared", "worktree"];
const TOOLS: &[&str] = &[
    "Read",
    "Write",
    "Edit",
    "MultiEdit",
    "NotebookEdit",
    "Glob",
    "Grep",
    "Bash",
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
    "Agent",
    "ApplyAgentChanges",
    "Verify",
    "LSP",
    "ListMcpResources",
    "ReadMcpResource",
];

struct Definition {
    label: String,
    meta: Mapping,
    body: String,
}

impl Definition {
    /// Checks that `text` is exactly `prompts/<area>/<name>.md`, then splits
    /// `---\n<yaml>\n---\n<body>` and parses the YAML.
    fn load(area: &str, name: &str, text: &str) -> Self {
        let label = format!("{area}/{name}.md");
        let path = format!("{}/prompts/{label}", env!("CARGO_MANIFEST_DIR"));
        let on_disk = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{path}: {err}"));
        assert_eq!(
            text, on_disk,
            "{label}: registry text differs from the file"
        );

        let rest = text
            .strip_prefix("---\n")
            .unwrap_or_else(|| panic!("{label}: must start with a `---` frontmatter fence"));
        let (yaml, body) = rest
            .split_once("\n---\n")
            .unwrap_or_else(|| panic!("{label}: frontmatter is never closed"));
        let meta = serde_yaml_ng::from_str(yaml)
            .unwrap_or_else(|err| panic!("{label}: invalid frontmatter: {err}"));
        assert!(!body.trim().is_empty(), "{label}: empty body");
        Self {
            label,
            meta,
            body: body.to_string(),
        }
    }

    fn assert_keys(&self, allowed: &[&str]) {
        for key in self.meta.keys() {
            let key = key
                .as_str()
                .unwrap_or_else(|| panic!("{}: non-string key", self.label));
            assert!(
                allowed.contains(&key),
                "{}: unknown key `{key}`",
                self.label
            );
        }
    }

    fn optional(&self, key: &str) -> Option<&str> {
        self.meta.get(key).map(|value| {
            value
                .as_str()
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| panic!("{}: `{key}` must be a non-empty string", self.label))
        })
    }

    fn required(&self, key: &str) -> &str {
        self.optional(key)
            .unwrap_or_else(|| panic!("{}: missing `{key}`", self.label))
    }

    fn assert_one_of(&self, key: &str, allowed: &[&str]) {
        if let Some(value) = self.optional(key) {
            assert!(
                allowed.contains(&value),
                "{}: `{key}: {value}` is not one of {allowed:?}",
                self.label
            );
        }
    }
}

/// Every entry of a comma-separated tool list names a known tool; permission
/// rules such as `Bash(git add:*)` count as their tool.
fn assert_known_tools(label: &str, list: &str) {
    for entry in list.split(',').map(str::trim) {
        let tool = entry.split_once('(').map_or(entry, |(tool, _)| tool);
        assert!(TOOLS.contains(&tool), "{label}: unknown tool `{entry}`");
    }
}

#[test]
fn agent_frontmatter_is_valid() {
    for (name, text) in agents::BUILTIN {
        let agent = Definition::load("agents", name, text);
        agent.assert_keys(AGENT_KEYS);
        assert_eq!(agent.required("name"), *name, "{}", agent.label);
        agent.required("description");
        let tools = agent.required("tools");
        if tools != "*" {
            assert_known_tools(&agent.label, tools);
        }
        agent.required("model");
        agent.assert_one_of("model", MODELS);
        agent.assert_one_of("permissionMode", PERMISSION_MODES);
        agent.assert_one_of("isolation", ISOLATIONS);
        agent.optional("color");
        if let Some(turns) = agent.meta.get("maxTurns") {
            assert!(
                turns.as_u64().is_some_and(|turns| turns > 0),
                "{}: `maxTurns` must be a positive integer",
                agent.label
            );
        }
    }
}

#[test]
fn command_frontmatter_is_valid() {
    for (name, text) in commands::BUILTIN {
        let command = Definition::load("commands", name, text);
        command.assert_keys(COMMAND_KEYS);
        command.required("description");
        command.required("argument-hint");
        if let Some(tools) = command.optional("allowed-tools") {
            assert_known_tools(&command.label, tools);
        }
        assert!(
            command.body.contains("$ARGUMENTS"),
            "{}: body never uses $ARGUMENTS",
            command.label
        );
    }
}
