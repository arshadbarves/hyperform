use super::*;

fn parse(text: &str) -> Rule {
    Rule::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn error(text: &str) -> String {
    match Rule::parse(text) {
        Ok(rule) => panic!("{text} parsed as {rule}"),
        Err(PolicyError::InvalidRule { rule, message }) => {
            assert_eq!(rule, text.trim());
            message
        }
        Err(other) => other.to_string(),
    }
}

#[test]
fn every_form_round_trips_through_display() {
    for text in [
        "Bash",
        "Bash(npm test:*)",
        "Bash(git status)",
        "Bash(make build && make test)",
        "Read",
        "Read(src/**)",
        "Edit(docs/**)",
        "Read(~/.ssh/**)",
        "Edit(/abs/path/**)",
        "Edit(//abs/path/**)",
        "Grep(src/**)",
        "Write(notes.md)",
        "WebFetch",
        "WebFetch(domain:example.com)",
        "WebFetch(domain:*.example.com)",
        "WebSearch",
        "Agent",
        "Agent(explore)",
        "Skill",
        "Skill(release-notes)",
        "mcp__github",
        "mcp__github__*",
        "mcp__github__create_issue",
        "TodoWrite",
        "ExitPlanMode",
    ] {
        let rule = parse(text);
        assert_eq!(rule.to_string(), text);
        assert_eq!(parse(&rule.to_string()), rule);
    }
}

#[test]
fn aliases_and_legacy_spellings_normalize() {
    assert_eq!(parse("Task(explore)").to_string(), "Agent(explore)");
    assert_eq!(parse("Task").to_string(), "Agent");
    assert_eq!(parse("Bash(cargo test*)").to_string(), "Bash(cargo test:*)");
    assert_eq!(parse("Bash(*)").to_string(), "Bash");
    assert_eq!(parse("Bash(:*)").to_string(), "Bash");
    assert_eq!(
        parse("  Bash( git status )  ").to_string(),
        "Bash(git status)"
    );
    assert_eq!(
        parse("WebFetch(domain:Example.COM)").to_string(),
        "WebFetch(domain:example.com)"
    );
    assert_eq!(parse("Bash(cargo test*)"), parse("Bash(cargo test:*)"));
}

#[test]
fn targets_are_typed() {
    assert!(matches!(
        parse("Read(src/**)").target,
        Target::Files {
            scope: FileScope::Read,
            pattern: Some(_)
        }
    ));
    assert!(matches!(
        parse("Edit").target,
        Target::Files {
            scope: FileScope::Edit,
            pattern: None
        }
    ));
    assert!(matches!(
        parse("Glob").target,
        Target::Files {
            scope: FileScope::Tool(_),
            pattern: None
        }
    ));
    assert!(matches!(parse("TodoWrite").target, Target::Tool(_)));
    assert!(matches!(
        parse("mcp__s").target,
        Target::Mcp {
            tool: McpTool::All { star: false },
            ..
        }
    ));
    assert!(
        matches!(parse("mcp__s__t__u").target, Target::Mcp { tool: McpTool::Named(ref t), .. } if t == "t__u")
    );
}

#[test]
fn malformed_rules_are_rejected_with_reasons() {
    assert_eq!(error(""), "missing tool name");
    assert_eq!(error("Bash("), "missing closing parenthesis");
    assert_eq!(error("Bash()"), "empty specifier");
    assert_eq!(error("(ls)"), "missing tool name");
    assert!(error("Bash tool").contains("tool names"));
    assert_eq!(
        error("WebSearch(query)"),
        "WebSearch rules do not take a specifier"
    );
    assert_eq!(
        error("TodoWrite(x)"),
        "TodoWrite rules do not take a specifier"
    );
    assert!(error("WebFetch(example.com)").contains("domain:"));
    assert!(error("Bash(a && b:*)").contains("one simple command"));
    assert!(error("Read(!secret)").contains("negated"));
    assert!(error("Todo*").contains("wildcards"));
    assert_eq!(error("mcp__"), "missing MCP server name");
    assert_eq!(error("mcp__*"), "missing MCP server name");
    assert_eq!(error("mcp__github__"), "missing MCP tool name");
    assert!(error("mcp__github__get_*").contains("wildcards"));
    assert!(error("mcp__github(x)").contains("specifier"));
}

#[test]
fn glob_errors_keep_their_source() {
    match Rule::parse("Read(src/[)") {
        Err(PolicyError::InvalidPattern { rule, .. }) => assert_eq!(rule, "Read(src/[)"),
        other => panic!("unexpected {other:?}"),
    }
}
