//! Extension parsing and discovery: frontmatter shapes, namespacing,
//! skills, rules, precedence across scopes, and reported bad files.

#[allow(dead_code)]
mod support;

use std::path::Path;

use support::{Fixture, fixture, write};
use z_engine_config::{
    ExtensionScope, ExtensionSource, Extensions, discover_extensions, load_skill_body, parse_agent,
    parse_command,
};
use z_engine_protocol::{Isolation, PermissionMode};

fn source(path: &str) -> ExtensionSource {
    ExtensionSource::new(ExtensionScope::Project, Path::new(path))
}

fn discover(f: &Fixture) -> Extensions {
    discover_extensions(&f.paths, &f.project, true)
}

#[test]
fn agent_tools_accept_a_string_or_a_list() {
    let text = "---\ndescription: Reviews diffs\ntools: Read, Grep Glob\ndisallowed_tools: [Bash]\n\
                model: fast\npermissionMode: acceptEdits\nisolation: worktree\nmaxTurns: 30\ncolor: blue\n---\n\n\
                You review code.\n";
    let agent = parse_agent(text, source("/a/reviewer.md")).unwrap();
    assert_eq!(agent.name, "reviewer");
    assert_eq!(
        agent.tools,
        Some(vec!["Read".into(), "Grep".into(), "Glob".into()])
    );
    assert_eq!(agent.disallowed_tools, ["Bash"]);
    assert_eq!(agent.model.as_deref(), Some("fast"));
    assert_eq!(agent.permission_mode, Some(PermissionMode::AcceptEdits));
    assert_eq!(
        (agent.isolation, agent.max_turns),
        (Isolation::Worktree, Some(30))
    );
    assert_eq!(agent.prompt, "You review code.");

    let listed = "---\nname: explorer\ndescription: Finds code\ntools:\n  - Read\n  - Bash(git log:*)\n---\nExplore.";
    let agent = parse_agent(listed, source("/a/other.md")).unwrap();
    assert_eq!(agent.name, "explorer");
    assert_eq!(
        agent.tools,
        Some(vec!["Read".into(), "Bash(git log:*)".into()])
    );
    assert_eq!(agent.isolation, Isolation::Shared);

    let all = parse_agent(
        "---\ndescription: d\ntools: \"*\"\n---\n",
        source("/a/x.md"),
    )
    .unwrap();
    assert_eq!(all.tools, None);
    let blank = parse_agent("---\ndescription: d\ntools: \"\"\n---\n", source("/a/x.md")).unwrap();
    assert_eq!(blank.tools, None);
}

#[test]
fn permission_mode_plan_means_planning_at_the_callers_level() {
    for name in ["plan", "readOnly"] {
        let text = format!("---\ndescription: d\npermissionMode: {name}\n---\n");
        let agent = parse_agent(&text, source("/a/x.md")).unwrap();
        assert!(agent.planning, "{name}");
        assert_eq!(agent.permission_mode, None, "{name}");
    }
    let level = parse_agent(
        "---\ndescription: d\npermissionMode: bypassPermissions\n---\n",
        source("/a/x.md"),
    )
    .unwrap();
    assert!(!level.planning);
    assert_eq!(level.permission_mode, Some(PermissionMode::Bypass));
}

#[test]
fn invalid_agents_are_errors() {
    let missing = parse_agent("---\nname: a\n---\nbody", source("/a/a.md"));
    assert!(missing.unwrap_err().contains("description"));
    let mode = parse_agent(
        "---\ndescription: d\npermissionMode: yolo\n---\n",
        source("/a/a.md"),
    );
    assert!(mode.unwrap_err().contains("yolo"));
    let yaml = parse_agent("---\ndescription: [broken\n---\n", source("/a/a.md"));
    assert!(yaml.unwrap_err().starts_with("invalid frontmatter"));
    let unclosed = parse_agent("---\ndescription: d\n", source("/a/a.md"));
    assert!(unclosed.is_err());
}

#[test]
fn commands_parse_claude_code_frontmatter() {
    let text = "---\nallowed-tools: Bash(git add:*), Bash(git commit:*)\nargument-hint: [message]\n\
                disable-model-invocation: true\nmodel: fast\n---\nCommit with $ARGUMENTS";
    let command = parse_command("git:commit", text, source("/c/git/commit.md")).unwrap();
    assert_eq!(
        command.allowed_tools,
        ["Bash(git add:*)", "Bash(git commit:*)"]
    );
    assert_eq!(command.argument_hint.as_deref(), Some("[message]"));
    assert!(command.disable_model_invocation);
    assert_eq!(command.description, "Commit with $ARGUMENTS");

    let long = format!("\n\n## {}\nmore", "x".repeat(150));
    let command = parse_command("long", &long, source("/c/long.md")).unwrap();
    assert_eq!(command.description.chars().count(), 100);
    assert!(command.description.ends_with('…'));
}

#[test]
fn discovery_namespaces_commands_and_reads_every_kind() {
    let f = fixture();
    let dir = f.project.join(".z-engine");
    write(
        &dir.join("commands/frontend/component.md"),
        "---\ndescription: New component\n---\nMake $1",
    );
    write(&dir.join("commands/review.md"), "Review the diff.");
    write(&dir.join("commands/notes.txt"), "ignored");
    write(
        &dir.join("skills/pdf/SKILL.md"),
        "---\ndescription: Read PDFs\nallowed-tools: [Bash]\n---\n# PDF\nSteps.",
    );
    write(&dir.join("skills/empty/README.md"), "no SKILL.md here");
    write(
        &dir.join("rules/rust.md"),
        "---\nglobs: src/**/*.rs, tests/**/*.{rs,toml}\n---\nUse thiserror.",
    );
    write(
        &dir.join("rules/style/always.md"),
        "---\nalwaysApply: true\ndescription: House style\n---\nBe brief.",
    );
    write(
        &dir.join("output-styles/terse.md"),
        "---\ndescription: Short answers\n---\nAnswer in one line.",
    );
    let found = discover(&f);
    assert!(found.errors.is_empty(), "{:?}", found.errors);

    let commands: Vec<_> = found
        .commands
        .iter()
        .map(|c| (c.name.as_str(), c.description.as_str()))
        .collect();
    assert_eq!(
        commands,
        [
            ("frontend:component", "New component"),
            ("review", "Review the diff.")
        ]
    );
    let skill = &found.skills[0];
    assert_eq!(
        (skill.name.as_str(), skill.description.as_str()),
        ("pdf", "Read PDFs")
    );
    assert_eq!(skill.allowed_tools, ["Bash"]);
    assert!(skill.dir.ends_with("pdf") && skill.file.ends_with("SKILL.md"));
    assert_eq!(load_skill_body(skill).unwrap(), "# PDF\nSteps.");
    let rules: Vec<_> = found
        .rules
        .iter()
        .map(|r| (r.name.as_str(), r.globs.len(), r.always_apply))
        .collect();
    assert_eq!(rules, [("rust", 2, false), ("style:always", 0, true)]);
    assert_eq!(found.output_styles[0].name, "terse");
    assert_eq!(found.output_styles[0].source.scope, ExtensionScope::Project);
}

#[test]
fn higher_scopes_replace_same_named_definitions() {
    let mut f = fixture();
    let home = f.tmp.path().join("home");
    f.paths.home_dir = Some(home.clone());
    let agent = |who: &str| format!("---\nname: helper\ndescription: from {who}\n---\n");
    let locations = [
        (home.join(".claude/agents/helper.md"), "claude-user"),
        (f.paths.config_dir.join("agents/helper.md"), "user"),
        (f.project.join(".claude/agents/helper.md"), "claude-project"),
        (f.project.join(".z-engine/agents/helper.md"), "project"),
    ];
    for (path, who) in &locations {
        write(path, &agent(who));
    }
    write(
        &home.join(".claude/commands/only-claude.md"),
        "From Claude.",
    );
    let expected = [
        ("project", ExtensionScope::Project),
        ("claude-project", ExtensionScope::ClaudeProject),
        ("user", ExtensionScope::User),
        ("claude-user", ExtensionScope::ClaudeUser),
    ];
    for (index, (who, scope)) in expected.iter().enumerate() {
        let found = discover(&f);
        assert_eq!(found.agents.len(), 1);
        assert_eq!(found.agents[0].description, format!("from {who}"));
        assert_eq!(found.agents[0].source.scope, *scope);
        assert_eq!(found.commands[0].source.scope, ExtensionScope::ClaudeUser);
        std::fs::remove_file(&locations[locations.len() - 1 - index].0).unwrap();
    }
}

#[test]
fn claude_folders_are_ignored_without_compat() {
    let mut f = fixture();
    let home = f.tmp.path().join("home");
    f.paths.home_dir = Some(home.clone());
    write(
        &home.join(".claude/agents/a.md"),
        "---\ndescription: d\n---\n",
    );
    write(&f.project.join(".claude/commands/c.md"), "c");
    write(
        &f.project.join(".claude/skills/s/SKILL.md"),
        "---\ndescription: d\n---\n",
    );
    let found = discover_extensions(&f.paths, &f.project, false);
    assert_eq!(found, Extensions::default());
}

#[test]
fn bad_files_are_reported_and_skipped() {
    let f = fixture();
    let dir = f.project.join(".z-engine");
    write(&dir.join("agents/good.md"), "---\ndescription: fine\n---\n");
    write(&dir.join("agents/no-description.md"), "---\nname: x\n---\n");
    write(
        &dir.join("agents/huge.md"),
        &format!("---\ndescription: d\n---\n{}", "x".repeat(300 * 1024)),
    );
    write(
        &dir.join("agents/twin.md"),
        "---\nname: good\ndescription: again\n---\n",
    );
    write(
        &dir.join("rules/bad-glob.md"),
        "---\nglobs: \"src/[unclosed\"\n---\n",
    );
    write(&dir.join("skills/nodesc/SKILL.md"), "---\nname: n\n---\n");
    let found = discover(&f);
    assert_eq!(found.agents.len(), 1);
    assert_eq!(found.agents[0].description, "fine");
    let mut failed: Vec<_> = found
        .errors
        .iter()
        .map(|e| {
            Path::new(&e.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    failed.sort();
    assert_eq!(
        failed,
        [
            "SKILL.md",
            "bad-glob.md",
            "huge.md",
            "no-description.md",
            "twin.md"
        ],
        "{:?}",
        found.errors
    );
}

#[cfg(unix)]
#[test]
fn project_files_linking_outside_the_project_are_skipped() {
    let f = fixture();
    let secret = f.tmp.path().join("secret.md");
    write(&secret, "---\ndescription: leaked\n---\n");
    let dir = f.project.join(".z-engine/agents");
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&secret, dir.join("leak.md")).unwrap();
    let found = discover(&f);
    assert!(found.agents.is_empty());
    assert!(found.errors[0].message.contains("outside the project"));
}
