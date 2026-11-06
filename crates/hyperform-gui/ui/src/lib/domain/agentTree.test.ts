import { describe, expect, it } from "vitest";
import { agentActivity, agentSections, agentTree, workCounts } from "./agentTree";
import type { ToolCallView } from "./sessionView/types";
import { agentInfo } from "./testFixtures";

describe("agentSections", () => {
  it("pins work waiting to be applied, then what runs, then what finished", () => {
    const agents = {
      a: agentInfo({ agentId: "a", status: "running", startedAt: 1 }),
      b: agentInfo({ agentId: "b", status: "completed", startedAt: 2 }),
      c: agentInfo({
        agentId: "c",
        status: "completed",
        startedAt: 3,
        worktree: { path: "/w", branch: "hyperform/c", base: "abc", state: "pending", filesChanged: 3, diffstat: "+20 −4" },
      }),
      d: agentInfo({ agentId: "d", status: "failed", startedAt: 4 }),
    };
    const sections = agentSections(agentTree(agents));
    expect(sections.ready.map((n) => n.info.agentId)).toEqual(["c"]);
    expect(sections.working.map((n) => n.info.agentId)).toEqual(["a"]);
    expect(sections.finished.map((n) => n.info.agentId)).toEqual(["b", "d"]);
  });

  it("keeps a helper that is still running out of Ready to apply", () => {
    const worktree = { path: "/w", branch: "hyperform/a", base: "abc", state: "pending" as const, filesChanged: 1, diffstat: "+2 −0" };
    const agents = { a: agentInfo({ agentId: "a", status: "running", startedAt: 1, worktree }) };
    const sections = agentSections(agentTree(agents));
    expect(sections.ready).toEqual([]);
    expect(sections.working.map((n) => n.info.agentId)).toEqual(["a"]);
    expect(workCounts(agents, {}).pendingWorktrees).toBe(0);
  });
});

describe("agentActivity", () => {
  const tool = (over: Partial<ToolCallView>): ToolCallView => ({
    callId: "c",
    agentId: "a",
    tool: "Read",
    title: "",
    input: { file_path: "src/auth.rs" },
    status: "running",
    summary: "",
    output: "",
    progress: "",
    durationMs: null,
    startedAt: 1,
    ...over,
  });

  it("says what a subagent is doing right now", () => {
    const tools = { x: tool({ callId: "x", startedAt: 1 }), y: tool({ callId: "y", tool: "Bash", input: { command: "npm test" }, startedAt: 2 }) };
    expect(agentActivity(tools, "a")).toBe("Running npm test");
  });

  it("is quiet when the agent runs no tool", () => {
    expect(agentActivity({ x: tool({ status: "ok" }) }, "a")).toBeNull();
    expect(agentActivity({ x: tool({ agentId: "b" }) }, "a")).toBeNull();
  });
});
