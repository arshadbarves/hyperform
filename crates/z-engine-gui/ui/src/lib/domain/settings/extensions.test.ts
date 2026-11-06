import { describe, expect, it } from "vitest";
import type { Extensions } from "../../protocol/config/Extensions";
import { markdownFromDef } from "./extensionMarkdown";
import {
  extensionEntries,
  extensionFileName,
  extensionNameError,
  extensionTemplate,
  isNativeScope,
  type ExtensionKind,
} from "./extensions";

describe("extensionFileName", () => {
  it("derives the write name from the source path", () => {
    expect(extensionFileName("agents", "/p/.hyperform/agents/reviewer.md")).toBe("reviewer");
    expect(extensionFileName("commands", "/p/.hyperform/commands/frontend/component.md")).toBe("frontend:component");
    expect(extensionFileName("rules", "C:\\Users\\me\\hyperform\\rules\\style.md")).toBe("style");
    expect(extensionFileName("skills", "/c/hyperform/skills/release/SKILL.md")).toBe("release");
    expect(extensionFileName("output-styles", "/c/hyperform/output-styles/terse.md")).toBe("terse");
  });

  it("returns null for paths outside the kind folder", () => {
    expect(extensionFileName("agents", "/p/other/reviewer.md")).toBeNull();
    expect(extensionFileName("skills", "/c/skills/a/b/SKILL.md")).toBeNull();
    expect(extensionFileName("agents", "/p/agents/readme.txt")).toBeNull();
  });
});

describe("extension names and templates", () => {
  it("allows folders only for commands and rules", () => {
    expect(extensionNameError("commands", "frontend:component")).toBeNull();
    expect(extensionNameError("agents", "frontend:component")).toContain("letters");
    expect(extensionNameError("skills", "")).toContain("Enter");
    expect(extensionNameError("rules", "a".repeat(65))).toContain("64");
  });

  it.each<ExtensionKind>(["agents", "commands", "skills", "rules", "output-styles"])("starts %s with frontmatter", (kind) => {
    const text = extensionTemplate(kind, "demo");
    expect(text.startsWith("---\n")).toBe(true);
    expect(text).toContain("description:");
    expect(text.split("\n---\n").length).toBe(2);
  });

  it("quotes a bracketed argument hint so YAML reads it as text", () => {
    expect(extensionTemplate("commands", "fix")).toContain(`argument-hint: "[target]"`);
  });
});

describe("extensionEntries", () => {
  const source = (path: string, scope: "user" | "claudeProject") => ({ scope, path });
  const extensions: Extensions = {
    agents: [
      {
        name: "code-reviewer",
        description: "Reviews diffs",
        tools: ["Read", "Bash(git diff:*)"],
        disallowedTools: [],
        model: "fast",
        permissionMode: null,
        planning: false,
        isolation: "shared",
        maxTurns: 12,
        color: null,
        prompt: "Review carefully.",
        source: source("/c/hyperform/agents/reviewer.md", "user"),
      },
    ],
    commands: [],
    skills: [],
    rules: [],
    outputStyles: [],
    errors: [],
  };

  it("keeps the file name separate from the definition name", () => {
    expect(extensionEntries(extensions, "agents")).toEqual([
      {
        kind: "agents",
        name: "code-reviewer",
        description: "Reviews diffs",
        source: source("/c/hyperform/agents/reviewer.md", "user"),
        fileName: "reviewer",
      },
    ]);
    expect(isNativeScope("user")).toBe(true);
    expect(isNativeScope("claudeProject")).toBe(false);
  });

  it("rebuilds a definition as markdown when the file cannot be read", () => {
    const text = markdownFromDef("agents", extensions.agents[0]);
    expect(text).toBe(
      `---\nname: code-reviewer\ndescription: Reviews diffs\ntools: ["Read", "Bash(git diff:*)"]\nmodel: fast\nmaxTurns: 12\n---\n\nReview carefully.\n`,
    );
    const command = markdownFromDef("commands", {
      name: "fix",
      description: "Fix: the bug",
      argumentHint: "[issue]",
      allowedTools: [],
      model: null,
      disableModelInvocation: true,
      body: "Fix $ARGUMENTS",
      source: source("/c/commands/fix.md", "user"),
    });
    expect(command).toContain(`description: "Fix: the bug"`);
    expect(command).toContain(`argument-hint: "[issue]"`);
    expect(command).toContain("disable-model-invocation: true");
  });
});
