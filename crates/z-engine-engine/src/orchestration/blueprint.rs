//! From an agent definition and its caller to the child's `AgentSpec`:
//! the definition prompt plus the subagent preamble, the filtered tools
//! (never the user-facing ones, no `Agent` at the depth limit), the model
//! (`inherit`, a role alias, or a literal id), the permission level,
//! planning, the turn budget and the isolation.

use std::path::PathBuf;

use z_engine_config::{AgentDef, ExtensionScope, Settings};
use z_engine_protocol::{AgentId, Isolation, PermissionMode};
use z_engine_tools::names;

use crate::run::{AgentSpec, ModelChoice, RunContext, ToolFilter, WorktreeScope};

/// Where the child works.
#[derive(Debug, Clone)]
pub(crate) struct Placement {
    pub root: PathBuf,
    pub worktree: Option<WorktreeScope>,
}

/// `routed`: the model the launch routed an inheriting definition to.
pub(crate) fn child_spec(
    def: &AgentDef,
    parent: &RunContext,
    agent_id: AgentId,
    placement: Placement,
    routed: Option<&str>,
) -> AgentSpec {
    let settings = parent.core.settings();
    let limits = &settings.settings.agents;
    let depth = parent.spec.depth + 1;
    let mut deny = def.disallowed_tools.clone();
    deny.extend([names::ASK_USER_QUESTION, names::EXIT_PLAN_MODE].map(String::from));
    if depth >= limits.max_depth {
        deny.push(names::AGENT.to_string());
    }
    AgentSpec {
        agent_id,
        base_prompt: format!(
            "{}\n\n{}",
            def.prompt.trim(),
            z_engine_prompts::system::SUBAGENT.trim()
        ),
        tools: ToolFilter {
            allow: def.tools.clone(),
            deny,
        },
        model: ModelChoice::Fixed(model_for(def, &settings.settings, &parent.model(), routed)),
        mode: child_mode(def, parent, settings.trusted),
        planning: child_planning(def, &parent.spec),
        max_turns: def.max_turns.unwrap_or(limits.max_turns).max(1),
        root: placement.root,
        depth,
        worktree: placement.worktree,
    }
}

/// The definition takes its caller's model (`inherit` or nothing), so its
/// launch may route it (`decisions_routing`).
pub(crate) fn inherits(def: &AgentDef) -> bool {
    matches!(
        def.model.as_deref().map(str::trim),
        None | Some("" | "inherit")
    )
}

/// `inherit` (or nothing) takes the caller's model, unless the launch
/// `routed` it; `main`, `fast` and `review` name the configured roles;
/// anything else is a model id.
pub(crate) fn model_for(
    def: &AgentDef,
    settings: &Settings,
    parent_model: &str,
    routed: Option<&str>,
) -> String {
    let named = def.model.as_deref().map(str::trim).unwrap_or_default();
    match named {
        "" | "inherit" => routed.unwrap_or(parent_model).to_string(),
        "main" => settings.model.main.clone(),
        "fast" => settings.model.fast_model().to_string(),
        "review" => settings.model.review_model().to_string(),
        id => id.to_string(),
    }
}

/// The caller's explicit choice wins over the definition's.
pub(crate) fn isolation(def: &AgentDef, requested: Option<Isolation>) -> Isolation {
    requested.unwrap_or(def.isolation)
}

/// A child plans when its definition says so or its caller's definition
/// chain does; while the session plans, every agent does
/// (`RunContext::planning`), so no definition can turn planning off.
fn child_planning(def: &AgentDef, parent: &AgentSpec) -> bool {
    def.planning || parent.planning
}

/// The definition's level, else the caller's; a session in bypass mode
/// still overrides it at each call (`RunContext::mode`). A definition
/// shipped by an untrusted project may not run looser than its caller (a
/// repository must not be able to start a `bypass` subagent). Planning is
/// separate and only ever restricts, so it needs no such check.
fn child_mode(def: &AgentDef, parent: &RunContext, trusted: bool) -> Option<PermissionMode> {
    let from_project = matches!(
        def.source.scope,
        ExtensionScope::Project | ExtensionScope::ClaudeProject
    );
    resolve_mode(
        def.permission_mode,
        parent.spec.mode,
        parent.mode(),
        trusted || !from_project,
    )
}

fn resolve_mode(
    requested: Option<PermissionMode>,
    inherited: Option<PermissionMode>,
    caller: PermissionMode,
    may_loosen: bool,
) -> Option<PermissionMode> {
    match requested {
        Some(mode) if may_loosen || looseness(mode) <= looseness(caller) => Some(mode),
        _ => inherited,
    }
}

fn looseness(mode: PermissionMode) -> u8 {
    match mode {
        PermissionMode::Default => 0,
        PermissionMode::AcceptEdits => 1,
        PermissionMode::Bypass => 2,
    }
}

#[cfg(test)]
mod tests {
    use z_engine_config::{ExtensionScope, ExtensionSource, parse_agent};

    use super::*;

    fn def(frontmatter: &str) -> AgentDef {
        let markdown = format!("---\nname: t\ndescription: T.\n{frontmatter}---\nBody.\n");
        let source = ExtensionSource {
            scope: ExtensionScope::Project,
            path: "/p/t.md".into(),
        };
        parse_agent(&markdown, source).unwrap()
    }

    #[test]
    fn models_resolve_inherit_aliases_and_ids() {
        let mut settings = Settings::default();
        settings.model.main = "main-model".into();
        settings.model.fast = Some("fast-model".into());
        assert_eq!(model_for(&def(""), &settings, "parent", None), "parent");
        assert_eq!(
            model_for(&def("model: inherit\n"), &settings, "parent", None),
            "parent"
        );
        assert_eq!(
            model_for(&def("model: fast\n"), &settings, "parent", None),
            "fast-model"
        );
        assert_eq!(
            model_for(&def("model: review\n"), &settings, "parent", None),
            "main-model"
        );
        assert_eq!(
            model_for(&def("model: vendor/x-1\n"), &settings, "parent", None),
            "vendor/x-1"
        );
    }

    #[test]
    fn only_inheriting_definitions_take_a_routed_model() {
        let settings = Settings::default();
        for frontmatter in ["", "model: inherit\n"] {
            assert!(inherits(&def(frontmatter)));
            let model = model_for(&def(frontmatter), &settings, "parent", Some("fast"));
            assert_eq!(model, "fast");
        }
        let pinned = def("model: vendor/x-1\n");
        assert!(!inherits(&pinned));
        assert_eq!(
            model_for(&pinned, &settings, "parent", Some("fast")),
            "vendor/x-1"
        );
    }

    #[test]
    fn untrusted_definitions_cannot_run_looser_than_the_caller() {
        use PermissionMode::{AcceptEdits, Bypass, Default};
        assert_eq!(resolve_mode(Some(Bypass), None, Default, false), None);
        assert_eq!(
            resolve_mode(Some(AcceptEdits), Some(Default), Default, false),
            Some(Default)
        );
        assert_eq!(
            resolve_mode(Some(Default), None, AcceptEdits, false),
            Some(Default)
        );
        assert_eq!(
            resolve_mode(Some(Bypass), None, Default, true),
            Some(Bypass)
        );
        assert_eq!(
            resolve_mode(None, Some(AcceptEdits), Bypass, false),
            Some(AcceptEdits)
        );
    }

    #[test]
    fn children_inherit_planning_and_cannot_turn_it_off() {
        let planner = AgentSpec {
            planning: true,
            ..AgentSpec::main("/p".into(), 1)
        };
        let builder = AgentSpec::main("/p".into(), 1);
        let plan = def("permissionMode: plan\n");
        assert!(plan.planning && plan.permission_mode.is_none());
        assert!(child_planning(&plan, &builder));
        for frontmatter in [
            "",
            "permissionMode: acceptEdits\n",
            "permissionMode: bypass\n",
        ] {
            assert!(child_planning(&def(frontmatter), &planner), "{frontmatter}");
            assert!(
                !child_planning(&def(frontmatter), &builder),
                "{frontmatter}"
            );
        }
    }

    #[test]
    fn requested_isolation_overrides_the_definition() {
        let worktree = def("isolation: worktree\n");
        assert_eq!(isolation(&worktree, None), Isolation::Worktree);
        assert_eq!(
            isolation(&worktree, Some(Isolation::Shared)),
            Isolation::Shared
        );
    }
}
