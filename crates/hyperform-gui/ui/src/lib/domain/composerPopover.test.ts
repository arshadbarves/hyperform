import { describe, expect, it } from "vitest";
import type { AgentCard } from "../commands/engine";
import { activePopover, agentMention, filterAgents, wrapIndex } from "./composerPopover";

const agent = (name: string, description = ""): AgentCard => ({
  name,
  description,
  source: "project",
  model: null,
  color: null,
});

describe("activePopover", () => {
  it("prefers the slash menu while typing a command name", () => {
    expect(activePopover("/rev", 4)).toEqual({ kind: "slash", query: "rev" });
    expect(activePopover("/review src", 11)).toBeNull();
  });

  it("offers memory targets for # on the first line", () => {
    expect(activePopover("# use pnpm", 5)).toEqual({ kind: "remember" });
    expect(activePopover("## heading", 3)).toBeNull();
    expect(activePopover("#a\nmore", 6)).toBeNull();
  });

  it("detects @ mentions at the caret", () => {
    expect(activePopover("look at @src/ma", 15)).toEqual({ kind: "mention", query: "src/ma" });
    expect(activePopover("mail me@x.com", 13)).toBeNull();
  });
});

describe("agent mentions", () => {
  it("filters by name or description and inserts the mention", () => {
    const agents = [agent("reviewer", "Reviews diffs"), agent("explore", "Reads code")];
    expect(filterAgents(agents, "agent-rev").map((a) => a.name)).toEqual(["reviewer"]);
    expect(filterAgents(agents, "code").map((a) => a.name)).toEqual(["explore"]);
    expect(filterAgents(agents, "")).toHaveLength(2);
    expect(agentMention(agents[0])).toBe("@agent-reviewer ");
  });

  it("wraps list selection", () => {
    expect(wrapIndex(0, -1, 3)).toBe(2);
    expect(wrapIndex(2, 1, 3)).toBe(0);
    expect(wrapIndex(5, 1, 0)).toBe(0);
  });
});
