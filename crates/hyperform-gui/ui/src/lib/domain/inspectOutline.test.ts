import { describe, expect, it } from "vitest";
import type { InspectRow } from "../promptInspectView";
import { outlineGroups, readerSource, stepSelection } from "./inspectOutline";

const msg = (role: string, label: string, content = "text", tokens = 10): InspectRow => ({
  key: `m-${label}`,
  kind: "msg",
  part: { role, label, content, tokens },
});
const tool = (name: string, tokens = 50): InspectRow => ({
  key: `t-${name}`,
  kind: "tool",
  tool: { name, description: `Runs ${name}`, schema: '{"type":"object"}', tokens },
});

const rows = [msg("system", "System"), msg("system", "Repo map"), msg("user", "User"), msg("assistant", "Assistant"), tool("Read"), tool("Bash")];

describe("outlineGroups", () => {
  it("groups the request by what each part is for, in a fixed order", () => {
    const groups = outlineGroups(rows, "", null);
    expect(groups.map((g) => [g.category, g.items.map((i) => i.index)])).toEqual([
      ["instructions", [0]],
      ["project", [1]],
      ["conversation", [2, 3]],
      ["capabilities", [4, 5]],
    ]);
    expect(groups.find((g) => g.category === "capabilities")?.tokens).toBe(100);
  });

  it("tells conversation parts apart by their first line", () => {
    const groups = outlineGroups([msg("user", "User", "\nFix the login loop\nmore"), msg("system", "System", "You are")], "", null);
    expect(groups.find((g) => g.category === "conversation")?.items[0]?.detail).toBe("Fix the login loop");
    expect(groups.find((g) => g.category === "instructions")?.items[0]?.detail).toBeNull();
    const result = outlineGroups([msg("tool", "Tool results", "← c1\nfn callback() {}")], "", null);
    expect(result[0]?.items[0]?.detail).toBe("fn callback() {}");
  });

  it("filters by search text and by one category", () => {
    expect(outlineGroups(rows, "bash", null).flatMap((g) => g.items.map((i) => i.label))).toEqual(["Bash"]);
    expect(outlineGroups(rows, "", "conversation").map((g) => g.category)).toEqual(["conversation"]);
  });
});

describe("stepSelection", () => {
  it("moves through what is visible and stops at the ends", () => {
    expect(stepSelection([2, 3, 5], 3, 1)).toBe(5);
    expect(stepSelection([2, 3, 5], 5, 1)).toBe(5);
    expect(stepSelection([2, 3, 5], 2, -1)).toBe(2);
    expect(stepSelection([2, 3, 5], 9, 1)).toBe(2);
    expect(stepSelection([], 0, 1)).toBeNull();
  });
});

describe("readerSource", () => {
  it("shows a tool's description with its schema as a JSON block", () => {
    expect(readerSource(tool("Read"))).toBe('Runs Read\n\n```json\n{"type":"object"}\n```');
  });

  it("keeps tool results as code so their output is not reflowed", () => {
    expect(readerSource(msg("tool", "Tool result", "a  b\n  c"))).toBe("```\na  b\n  c\n```");
    expect(readerSource(msg("user", "User", "**hi**"))).toBe("**hi**");
  });
});
