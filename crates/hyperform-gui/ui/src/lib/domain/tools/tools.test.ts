import { describe, expect, it } from "vitest";
import { parseGitDiff } from "../../diffParse";
import { editDiff, lineDiff, unifiedHunks } from "./editDiff";
import { compactJson, relPath, str } from "./toolInput";
import { toolMeta, toolSubject } from "./toolMeta";

describe("toolMeta", () => {
  it("classifies Claude Code tool names", () => {
    expect(toolMeta("MultiEdit").family).toBe("edit");
    expect(toolMeta("Write").family).toBe("write");
    expect(toolMeta("Grep").family).toBe("search");
    expect(toolMeta("Task").family).toBe("agent");
    expect(toolMeta("SomethingNew")).toEqual({ family: "generic", label: "SomethingNew" });
  });

  it("splits MCP tool names into server and tool", () => {
    expect(toolMeta("mcp__github__create_issue")).toEqual({
      family: "mcp",
      label: "create_issue",
      server: "github",
    });
  });
});

describe("toolSubject", () => {
  it("summarizes the most useful argument", () => {
    expect(toolSubject("Read", { file_path: "src/a.rs", offset: 10, limit: 5 })).toBe("src/a.rs:10–14");
    expect(toolSubject("Bash", { command: "npm test\n&& echo" })).toBe("npm test");
    expect(toolSubject("Grep", { pattern: "TODO", path: "src" })).toBe("TODO in src");
    expect(toolSubject("TodoWrite", { todos: [{}, {}] })).toBe("2 items");
    expect(toolSubject("WebSearch", { query: "svelte runes" })).toBe("svelte runes");
    expect(toolSubject("mcp__x__y", { a: 1 })).toBe('{"a":1}');
  });
});

describe("toolInput", () => {
  it("reads strings defensively", () => {
    expect(str({ a: 1, b: "x" }, "a", "b")).toBe("x");
    expect(str(null, "a")).toBe("");
    expect(compactJson({})).toBe("");
    expect(compactJson({ k: "v".repeat(200) }, 20)).toHaveLength(20);
  });

  it("shortens paths inside the project root", () => {
    expect(relPath("/repo/src/a.ts", "/repo/")).toBe("src/a.ts");
    expect(relPath("/elsewhere/a.ts", "/repo")).toBe("/elsewhere/a.ts");
  });
});

describe("editDiff", () => {
  it("diffs lines with an LCS", () => {
    const ops = lineDiff(["a", "b", "c"], ["a", "x", "c", "d"]);
    expect(ops.map((o) => `${o.kind}:${o.text}`)).toEqual(["ctx:a", "del:b", "add:x", "ctx:c", "add:d"]);
  });

  it("keeps a few lines of context per hunk", () => {
    const before = Array.from({ length: 20 }, (_, i) => `l${i}`);
    const after = before.map((l, i) => (i === 2 || i === 17 ? `${l}!` : l));
    const hunks = unifiedHunks(lineDiff(before, after));
    expect(hunks.filter((h) => h.startsWith("@@"))).toEqual(["@@ -1,6 +1,6 @@", "@@ -15,6 +15,6 @@"]);
  });

  it("builds a parseable diff for Edit", () => {
    const diff = editDiff("Edit", { file_path: "src/a.ts", old_string: "let a = 1;\n", new_string: "const a = 1;\n" });
    const parsed = parseGitDiff(diff ?? "");
    expect(parsed.path).toBe("src/a.ts");
    expect(parsed.added).toBe(1);
    expect(parsed.deleted).toBe(1);
  });

  it("concatenates MultiEdit hunks and renders Write as a new file", () => {
    const multi = editDiff("MultiEdit", {
      file_path: "f",
      edits: [
        { old_string: "a", new_string: "b" },
        { old_string: "c", new_string: "d" },
      ],
    });
    expect(parseGitDiff(multi ?? "").added).toBe(2);
    const created = parseGitDiff(editDiff("Write", { file_path: "n.txt", content: "one\ntwo\n" }) ?? "");
    expect(created).toMatchObject({ path: "n.txt", added: 2, deleted: 0 });
  });

  it("uses the display path in headers", () => {
    const diff = editDiff("Write", { file_path: "/repo/src/new.ts", content: "x" }, "src/new.ts") ?? "";
    expect(parseGitDiff(diff).path).toBe("src/new.ts");
  });

  it("returns null when nothing changed", () => {
    expect(editDiff("Edit", { file_path: "f", old_string: "same", new_string: "same" })).toBeNull();
    expect(editDiff("Edit", {})).toBeNull();
  });
});
