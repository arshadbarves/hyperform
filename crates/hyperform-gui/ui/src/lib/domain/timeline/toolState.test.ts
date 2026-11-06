import { describe, expect, it } from "vitest";
import type { ToolCallView } from "../sessionView/types";
import { agentInfo, assistant, text, toolResult, toolUse, user } from "../testFixtures";
import { hasVisibleUserContent, pairResults, visibleText, type ToolUseRef } from "./blocks";
import { linkAgentCalls, liveStreams, resolveToolCall } from "./toolState";

const use = { callId: "c1", name: "Bash", input: { command: "ls" } };

function live(over: Partial<ToolCallView> = {}): ToolCallView {
  return {
    callId: "c1",
    agentId: "main",
    tool: "Bash",
    title: "Run ls",
    input: { command: "ls" },
    status: "running",
    summary: "",
    output: "",
    progress: "a\n",
    durationMs: null,
    startedAt: 5,
    ...over,
  };
}

describe("pairResults", () => {
  it("indexes tool results by call id", () => {
    const results = pairResults([
      assistant("a1", [toolUse("c1", "Bash")]),
      user("r1", [toolResult("c1", "out"), toolResult("c2", "bad", true)]),
    ]);
    expect(results.c1).toEqual({ text: "out", images: [], isError: false });
    expect(results.c2.isError).toBe(true);
  });
});

describe("visible user content", () => {
  it("ignores reminders and tool payloads", () => {
    const message = user("u", [text("<system-reminder>x</system-reminder>"), text("real"), toolResult("c", "r")]);
    expect(visibleText(message)).toBe("real");
    expect(hasVisibleUserContent(user("r", [toolResult("c", "r")]))).toBe(false);
  });
});

describe("resolveToolCall", () => {
  it("prefers live status and output", () => {
    const state = resolveToolCall(use, { text: "final", images: [], isError: false }, live({ status: "ok", output: "live" }), true);
    expect(state).toMatchObject({ status: "ok", output: "live", title: "Run ls", progress: "a\n", startedAt: 5 });
  });

  it("falls back to the transcript result", () => {
    expect(resolveToolCall(use, { text: "err", images: [], isError: true }, undefined, false)).toMatchObject({
      status: "error",
      output: "err",
      durationMs: null,
    });
  });

  it("lets a persisted result settle a stale running call", () => {
    const state = resolveToolCall(use, { text: "done", images: [], isError: false }, live(), true);
    expect(state.status).toBe("ok");
    expect(state.progress).toBe("a\n");
  });

  it("infers running or cancelled for unanswered calls", () => {
    expect(resolveToolCall(use, undefined, undefined, true).status).toBe("running");
    expect(resolveToolCall(use, undefined, undefined, false).status).toBe("cancelled");
  });
});

describe("liveStreams", () => {
  it("returns one agent's streams in start order", () => {
    const streams = liveStreams({
      b: { messageId: "b", agentId: "main", text: "", thinking: "", startedAt: 2 },
      a: { messageId: "a", agentId: "main", text: "", thinking: "", startedAt: 1 },
      s: { messageId: "s", agentId: "agt", text: "", thinking: "", startedAt: 0 },
    });
    expect(streams.map((s) => s.messageId)).toEqual(["a", "b"]);
  });
});

describe("linkAgentCalls", () => {
  it("matches spawned agents by description and type", () => {
    const uses: ToolUseRef[] = [
      { callId: "c1", name: "Agent", input: { description: "Review", subagent_type: "review" } },
      { callId: "c2", name: "Agent", input: { description: "Explore", subagent_type: "explore" } },
      { callId: "c3", name: "Read", input: {} },
    ];
    const agents = [
      agentInfo({ agentId: "x", description: "Explore", agentType: "explore", startedAt: 1 }),
      agentInfo({ agentId: "y", description: "Review", agentType: "review", startedAt: 2 }),
    ];
    expect(linkAgentCalls(uses, agents)).toEqual({ c1: "y", c2: "x" });
  });

  it("prefers the recorded call id over matching descriptions", () => {
    const uses: ToolUseRef[] = [
      { callId: "c1", name: "Agent", input: { description: "Same", subagent_type: "explore" } },
      { callId: "c2", name: "Agent", input: { description: "Same", subagent_type: "explore" } },
    ];
    const agents = [
      agentInfo({ agentId: "x", callId: "c2", description: "Same", agentType: "explore", startedAt: 1 }),
      agentInfo({ agentId: "y", callId: "c1", description: "Same", agentType: "explore", startedAt: 2 }),
    ];
    expect(linkAgentCalls(uses, agents)).toEqual({ c1: "y", c2: "x" });
  });

  it("pairs leftovers by order and ignores other parents", () => {
    const uses = [
      { callId: "c1", name: "Task", input: { description: "A" } },
      { callId: "c2", name: "Task", input: { description: "B" } },
    ];
    const agents = [
      agentInfo({ agentId: "x", description: "renamed", startedAt: 1 }),
      agentInfo({ agentId: "nested", parentId: "x", description: "B", startedAt: 2 }),
      agentInfo({ agentId: "z", parentId: null, description: "other", startedAt: 3 }),
    ];
    expect(linkAgentCalls(uses, agents)).toEqual({ c1: "x", c2: "z" });
  });
});
