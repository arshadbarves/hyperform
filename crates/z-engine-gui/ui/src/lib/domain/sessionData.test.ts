import { describe, expect, it } from "vitest";
import type { SessionSummary } from "../protocol/SessionSummary";
import { agentTree, agentUsageRows, workCounts } from "./agentTree";
import { contextMeter } from "./contextMeter";
import { parseInspectRequest } from "./requestInspect";
import { fallbackTitle, listItems, viewTitle } from "./sessionList";
import { sidebarMark, unreadSessionOutcome } from "./sessionOutcome";
import { emptyView } from "./sessionView";
import { agentInfo, info, jobInfo, user, usage } from "./testFixtures";
import { cacheShare, usageLine } from "./usage";

function summary(over: Partial<SessionSummary> = {}): SessionSummary {
  return {
    sessionId: "S1",
    title: null,
    projectRoot: "/repo",
    createdAt: 1,
    updatedAt: 10,
    messageCount: 2,
    costUsd: 0.1,
    lastOutcome: null,
    legacy: false,
    ...over,
  };
}

describe("agent tree", () => {
  it("nests children under their parent in start order", () => {
    const tree = agentTree({
      b: agentInfo({ agentId: "b", startedAt: 2 }),
      a: agentInfo({ agentId: "a", startedAt: 1 }),
      c: agentInfo({ agentId: "c", parentId: "a", startedAt: 3 }),
    });
    expect(tree.map((n) => [n.info.agentId, n.depth])).toEqual([
      ["a", 0],
      ["c", 1],
      ["b", 0],
    ]);
    expect(tree[0].childCount).toBe(1);
  });

  it("counts running work and pending worktrees", () => {
    const worktree = { path: "/w", branch: "b", base: "abc", state: "pending" as const, filesChanged: 2, diffstat: "" };
    const counts = workCounts(
      { a: agentInfo({ agentId: "a" }), d: agentInfo({ agentId: "d", status: "completed", worktree }) },
      { j: jobInfo(), k: jobInfo({ jobId: "k", status: "completed" }) },
    );
    expect(counts).toEqual({ runningAgents: 1, runningJobs: 1, pendingWorktrees: 1 });
  });

  it("splits session cost between main and subagents", () => {
    const rows = agentUsageRows(
      { a: agentInfo({ agentId: "a", costUsd: 0.25 }) },
      { main: usage({ outputTokens: 10 }) },
      1,
    );
    expect(rows.map((r) => [r.agentId, r.costUsd])).toEqual([
      ["main", 0.75],
      ["a", 0.25],
    ]);
  });
});

describe("usage", () => {
  it("formats prompt and output tokens", () => {
    const u = usage({ inputTokens: 1000, cacheReadTokens: 11_000, outputTokens: 1400 });
    expect(usageLine(u)).toBe("12k in · 1.4k out");
    expect(usageLine(usage())).toBe("");
    expect(cacheShare(u)).toBeCloseTo(11 / 12);
  });
});

describe("session list", () => {
  it("prefers the engine title, then the first prompt", () => {
    const view = { ...emptyView("S1"), messages: [user("u1", "Fix the flaky login test please")] };
    expect(viewTitle(view)).toBe("Fix the flaky login test please");
    expect(viewTitle({ ...view, title: "Login flake" })).toBe("Login flake");
    expect(fallbackTitle("x".repeat(60))).toHaveLength(49);
  });

  it("merges live sessions and hides empty ones", () => {
    const live = { ...emptyView("S2"), info: info({ sessionId: "S2", updatedAt: 50 }), messages: [user("u", "new", 60)] };
    const items = listItems([summary(), summary({ sessionId: "S3", messageCount: 0 })], { S2: live });
    expect(items.map((i) => i.sessionId)).toEqual(["S2", "S1"]);
    expect(items[0]).toMatchObject({ title: "new", updatedAt: 60 });
    expect(items[1].title).toBe("New chat");
  });
});

const mark = (over: object) => ({
  outcome: { type: "completed" as const },
  verification: { status: "notApplicable" as const },
  at: 1,
  ...over,
});

describe("unread outcomes", () => {
  it("tints verified and failing results only", () => {
    expect(unreadSessionOutcome(mark({ verification: { status: "verified", checks: [] } }), false, null)?.tone).toBe(
      "verified",
    );
    expect(unreadSessionOutcome(mark({ outcome: { type: "failed", message: "x" } }), false, null)?.tone).toBe("warn");
    expect(unreadSessionOutcome(mark({}), false, null)).toEqual({ label: "Finished", tone: "neutral" });
  });

  it("hides the dot for active or busy sessions", () => {
    expect(unreadSessionOutcome(mark({}), true, null)).toBeNull();
    expect(unreadSessionOutcome(mark({}), false, "working")).toBeNull();
    expect(unreadSessionOutcome(null, false, null)).toBeNull();
  });
});

describe("sidebarMark", () => {
  const base = { active: false, activity: null, unread: null, lastOutcome: null };

  it("leaves the active chat to the title bar", () => {
    expect(sidebarMark({ ...base, active: true, activity: "approval" })).toBeNull();
  });

  it("puts needs-you above working above unread results", () => {
    const unread = mark({});
    expect(sidebarMark({ ...base, activity: "approval", unread })?.label).toBe("Needs you");
    expect(sidebarMark({ ...base, activity: "working", unread })?.tone).toBe("working");
    expect(sidebarMark({ ...base, unread: mark({ verification: { status: "verified", checks: [] } }) })?.tone).toBe("ok");
  });

  it("marks settled chats only when their last response did not complete", () => {
    expect(sidebarMark({ ...base, lastOutcome: { type: "failed", message: "x" } })).toEqual({
      tone: "danger",
      label: "Last response failed",
    });
    expect(sidebarMark({ ...base, lastOutcome: { type: "completed" } })).toBeNull();
    expect(sidebarMark(base)).toBeNull();
  });
});

describe("contextMeter", () => {
  it("scales the breakdown to the live token count", () => {
    const meter = contextMeter({
      contextTokens: 2000,
      contextLimit: 10_000,
      breakdown: { system: 100, tools: 300, instructions: 100, messages: 500, total: 1000, limit: 10_000 },
    });
    expect(meter.slices.map((s) => [s.id, s.tokens])).toEqual([
      ["system", 200],
      ["tools", 600],
      ["instructions", 200],
      ["messages", 1000],
    ]);
    expect(meter).toMatchObject({ used: 2000, pct: 20, level: "ok", totalOnly: false });
  });

  it("shows a single slice without a breakdown", () => {
    const meter = contextMeter({ contextTokens: 9000, contextLimit: 10_000, breakdown: null });
    expect(meter).toMatchObject({ pct: 90, level: "danger", totalOnly: true, remaining: 1000 });
    expect(meter.slices).toHaveLength(1);
  });
});

describe("parseInspectRequest", () => {
  it("reads the engine request shape", () => {
    const parsed = parseInspectRequest({
      model: "anthropic/claude-sonnet-4",
      system: [{ text: "You are Z.", cache: true }, { text: "# Environment\ncwd: /repo", cache: false }],
      messages: [
        { id: "u1", role: "user", content: [{ type: "text", text: "hi" }], createdAt: 1 },
        { id: "a1", role: "assistant", content: [{ type: "toolUse", id: "c1", name: "Read", input: { file_path: "a" } }] },
        { id: "r1", role: "user", content: [{ type: "toolResult", toolUseId: "c1", content: [{ type: "text", text: "x" }], isError: false }] },
      ],
      tools: [{ name: "Read", description: "Read a file", inputSchema: { type: "object" } }],
    });
    expect(parsed?.model).toBe("anthropic/claude-sonnet-4");
    expect(parsed?.messages.map((m) => m.label)).toEqual([
      "System",
      "Environment",
      "User",
      "Assistant · tool calls",
      "Tool results",
    ]);
    expect(parsed?.tools[0]).toMatchObject({ name: "Read", description: "Read a file" });
    expect(parsed?.totalTokens).toBeGreaterThan(0);
  });

  it("accepts OpenAI-style requests and rejects junk", () => {
    const parsed = parseInspectRequest({
      model: "gpt",
      messages: [
        { role: "system", content: "rules" },
        { role: "assistant", content: null, tool_calls: [{ function: { name: "Bash", arguments: "{}" } }] },
      ],
      tools: [{ type: "function", function: { name: "Bash", parameters: {} } }],
    });
    expect(parsed?.messages.map((m) => m.role)).toEqual(["system", "assistant"]);
    expect(parsed?.messages[1].content).toContain("→ Bash");
    expect(parsed?.tools.map((t) => t.name)).toEqual(["Bash"]);
    expect(parseInspectRequest(null)).toBeNull();
    expect(parseInspectRequest("nope")).toBeNull();
  });
});
