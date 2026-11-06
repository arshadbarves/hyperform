import { describe, expect, it } from "vitest";
import type { SlashCommandInfo } from "../commands/engine";
import { groupCommands, kindTag, LOCAL_UI_COMMANDS, menuOrder, mergeCommands, sourceTag } from "./slashCommands";

const cmd = (name: string, kind: SlashCommandInfo["kind"], source: SlashCommandInfo["source"] = "builtin") => ({
  name,
  description: `${name} command`,
  argumentHint: null,
  source,
  kind,
});

const list: SlashCommandInfo[] = [
  cmd("compact", "engine"),
  cmd("mcp", "engine"),
  cmd("review", "prompt"),
  cmd("ship", "prompt", "project"),
  cmd("mcp__fake__review", "prompt", "mcp"),
  cmd("help", "ui"),
];

describe("slash menu grouping", () => {
  it("groups prompts, then session commands, then app commands", () => {
    const groups = groupCommands(list);
    expect(groups.map((g) => g.label)).toEqual(["Prompts", "Session", "App"]);
    expect(groups[0].commands.map((c) => c.name)).toEqual(["review", "ship", "mcp__fake__review"]);
    expect(menuOrder(list).map((c) => c.name)).toEqual([
      "review",
      "ship",
      "mcp__fake__review",
      "compact",
      "mcp",
      "help",
    ]);
  });

  it("drops empty groups", () => {
    expect(groupCommands([cmd("help", "ui")]).map((g) => g.kind)).toEqual(["ui"]);
    expect(groupCommands([])).toEqual([]);
  });

  it("tags kind and source", () => {
    expect(kindTag(list[0])).toBe("ENGINE");
    expect(kindTag(list[3])).toBe("PROMPT");
    expect(sourceTag(list[3])).toBe("PROJECT");
    expect(sourceTag(list[4])).toBe("MCP");
    expect(sourceTag(list[2])).toBe("BUILT-IN");
    expect(sourceTag(list[5])).toBe("APP");
  });

  it("lets the engine's /mcp win over any local app command", () => {
    expect(LOCAL_UI_COMMANDS.some((c) => c.name === "mcp")).toBe(false);
    const merged = mergeCommands([cmd("mcp", "engine")]);
    expect(merged.filter((c) => c.name === "mcp")).toEqual([cmd("mcp", "engine")]);
  });
});
