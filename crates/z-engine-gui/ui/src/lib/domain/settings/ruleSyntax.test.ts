import { describe, expect, it } from "vitest";
import { ruleHint } from "./ruleSyntax";

const level = (rule: string) => ruleHint(rule).level;

describe("ruleHint", () => {
  it("offers examples for an empty rule", () => {
    expect(ruleHint("  ")).toMatchObject({ level: "info", message: expect.stringContaining("Bash(npm test:*)") });
  });

  it.each([
    "Read",
    "Bash",
    "Bash(npm test:*)",
    "Bash(git status)",
    "Edit(src/**)",
    "Read(~/.ssh/**)",
    "Edit(//abs/path/**)",
    "Grep(src/**)",
    "Write(notes.md)",
    "WebFetch",
    "WebFetch(domain:example.com)",
    "WebFetch(domain:*.example.com)",
    "WebSearch",
    "Agent(explore)",
    "Skill(release-notes)",
    "mcp__github",
    "mcp__github__*",
    "mcp__github__create_issue",
    "TodoWrite",
  ])("accepts %s", (rule) => {
    expect(level(rule)).toBe("ok");
  });

  it.each([
    ["Bash(npm test", "closing parenthesis"],
    ["Bash()", "Empty specifier"],
    ["(ls)", "Missing tool name"],
    ["Ba sh", "letters, digits"],
    ["Bash*", "mcp__server__*"],
    ["mcp__github(x)", "do not take a specifier"],
    ["mcp__github__", "Missing MCP tool name"],
    ["mcp__github__create*", "Only mcp__server__*"],
    ["mcp____tool", "Missing MCP server name"],
    ["WebFetch(example.com)", "domain:<host>"],
    ["WebFetch(domain:exa mple.com)", "Invalid domain"],
    ["WebSearch(foo)", "do not take a specifier"],
    ["TodoWrite(x)", "do not take a specifier"],
    ["Read(!secret)", "Negated"],
    ["Read(~user/x)", "~/"],
    ["Edit(src/[ab)", "Unbalanced"],
    ["Bash(make && make test:*)", "one simple command"],
  ])("rejects %s", (rule, message) => {
    expect(ruleHint(rule)).toMatchObject({ level: "error", message: expect.stringContaining(message) });
  });

  it("explains legacy prefixes, wildcard specifiers and single-slash paths", () => {
    expect(ruleHint("Bash(cargo test*)")).toMatchObject({ level: "info", message: expect.stringContaining("Bash(cargo test:*)") });
    expect(ruleHint("Bash(*)").message).toContain("every shell command");
    expect(ruleHint("Bash(:*)").message).toContain("every shell command");
    expect(level("Edit(/abs/**)")).toBe("info");
    expect(level("Bash(make build && make test)")).toBe("info");
  });

  it("warns about tool-name case and flags unknown tools softly", () => {
    expect(ruleHint("bash")).toMatchObject({ level: "warn", message: expect.stringContaining("Bash") });
    expect(ruleHint("MyTool")).toMatchObject({ level: "info", message: expect.stringContaining("exactly") });
  });

  it("trims whitespace like the policy engine", () => {
    expect(level("  Bash( git status )  ")).toBe("ok");
  });
});
