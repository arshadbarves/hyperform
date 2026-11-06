import { describe, expect, it } from "vitest";
import {
  ISLAND_H,
  ISLAND_HEAD_H,
  contextBubble,
  islandAction,
  islandCard,
  islandLine,
  islandMode,
  islandPlanRow,
  islandShape,
  noticeOpensIsland,
  recentSteps,
} from "./island";
import type { LiveStatus } from "./liveStatus";
import type { ToolCallView } from "./sessionView/types";
import { task, work } from "./work/fixtures";

function status(over: Partial<LiveStatus> = {}): LiveStatus {
  return {
    kind: "idle",
    tone: "quiet",
    activity: null,
    text: null,
    detail: null,
    progress: null,
    elapsed: null,
    cost: null,
    noticeId: null,
    waiting: null,
    moreWaiting: 0,
    helpers: { running: 0, pending: 0 },
    ...over,
  };
}

function tool(over: Partial<ToolCallView> = {}): ToolCallView {
  return {
    callId: "c1",
    agentId: "main",
    tool: "Read",
    title: "Read a.rs",
    input: { file_path: "src/a.rs" },
    status: "ok",
    summary: "",
    output: "",
    progress: "",
    durationMs: 10,
    startedAt: 1,
    ...over,
  };
}

describe("islandLine", () => {
  it("names the chat while idle and says nothing without one", () => {
    expect(islandLine(status(), "Fix login")).toEqual({ text: "Fix login", metric: null });
    expect(islandLine(status(), null)).toEqual({ text: null, metric: null });
  });

  it("keeps one number: the clock while working, the duration when done", () => {
    const working = status({ kind: "working", text: "Reading a.rs", elapsed: "8s", cost: "$0.01", detail: "x" });
    expect(islandLine(working, "Fix login")).toEqual({ text: "Reading a.rs", metric: "8s" });
    const done = status({ kind: "done", text: "Verified", elapsed: "1m 02s", cost: "$0.12" });
    expect(islandLine(done, null)).toEqual({ text: "Verified", metric: "1m 02s" });
  });

  it("counts down a retry and shows notices without a number", () => {
    expect(islandLine(status({ kind: "retrying", text: "Provider busy", detail: "retry 2 in 3s" }), null).metric).toBe(
      "retry 2 in 3s",
    );
    expect(islandLine(status({ kind: "notice", text: "Saved", detail: "x", elapsed: "1s" }), null)).toEqual({
      text: "Saved",
      metric: null,
    });
  });
});

describe("islandMode", () => {
  it("rests while idle and goes live for anything it is saying", () => {
    expect(islandMode(status(), false)).toBe("rest");
    for (const kind of ["working", "retrying", "notice", "done", "recap"] as const) {
      expect(islandMode(status({ kind }), false)).toBe("live");
    }
  });

  it("alerts when this chat needs you, and the card wins while open", () => {
    expect(islandMode(status({ kind: "attention" }), false)).toBe("alert");
    expect(islandMode(status({ kind: "attention" }), true)).toBe("expanded");
    expect(islandMode(status(), true)).toBe("expanded");
  });
});

describe("islandAction", () => {
  it("names the step waiting for you", () => {
    expect(islandAction(status({ kind: "attention", activity: "question" }))?.label).toBe("Answer");
    expect(islandAction(status({ kind: "attention", activity: "approval" }))?.label).toBe("Approve");
    expect(islandAction(status({ kind: "attention", activity: "plan" }))?.label).toBe("Review");
  });

  it("says where it goes and shows nothing unless this chat needs you", () => {
    expect(islandAction(status({ kind: "attention", activity: "approval" }))?.hint).toBe("Go to the approval request");
    expect(islandAction(status({ kind: "working", activity: "question" }))).toBeNull();
    expect(islandAction(status())).toBeNull();
  });
});

describe("islandShape", () => {
  const room = { capsule: 220, card: 180, viewportW: 1280, viewportH: 800 };

  it("is a capsule as wide as its words, never narrower than the pet", () => {
    expect(islandShape("live", room)).toEqual({ width: 220, height: ISLAND_H, radius: ISLAND_H / 2 });
    expect(islandShape("rest", { ...room, capsule: 0 }).width).toBe(ISLAND_H);
  });

  it("grows into a card as tall as its body", () => {
    expect(islandShape("expanded", room)).toEqual({ width: 400, height: ISLAND_HEAD_H + 180, radius: 22 });
  });

  it("keeps the card inside the window, where its body scrolls", () => {
    const small = { ...room, card: 900, viewportW: 360, viewportH: 400 };
    expect(islandCard(small)).toEqual({ width: 336, bodyMax: 320 - ISLAND_HEAD_H });
    expect(islandShape("expanded", small)).toMatchObject({ width: 336, height: 320 });
    expect(islandCard({ viewportW: 1600, viewportH: 1200 }).bodyMax).toBe(560 - ISLAND_HEAD_H);
  });
});

describe("noticeOpensIsland", () => {
  it("opens for text that does not fit, errors and notices with actions", () => {
    expect(noticeOpensIsland({ tone: "info" }, true)).toBe(true);
    expect(noticeOpensIsland({ tone: "error" }, false)).toBe(true);
    expect(noticeOpensIsland({ tone: "ok", actions: [{ label: "Undo" }] }, false)).toBe(true);
  });

  it("stays compact for a short routine notice or none", () => {
    expect(noticeOpensIsland({ tone: "ok" }, false)).toBe(false);
    expect(noticeOpensIsland({ tone: "warn", actions: [] }, false)).toBe(false);
    expect(noticeOpensIsland(null, true)).toBe(false);
  });
});

describe("contextBubble", () => {
  it("shows the number only once the context is filling up", () => {
    expect(contextBubble({ pct: 42, level: "ok" })).toEqual({ tone: "quiet", label: null });
    expect(contextBubble({ pct: 70, level: "warn" })).toEqual({ tone: "warn", label: "70%" });
    expect(contextBubble({ pct: 91, level: "danger" })).toEqual({ tone: "danger", label: "91%" });
  });
});

describe("recentSteps", () => {
  it("lists the main agent's latest steps, newest first", () => {
    const tools = {
      a: tool({ callId: "a", startedAt: 1 }),
      b: tool({ callId: "b", tool: "Bash", input: { command: "npm test" }, status: "running", startedAt: 3 }),
      c: tool({ callId: "c", agentId: "agt_1", startedAt: 4 }),
      d: tool({ callId: "d", status: "error", startedAt: 2 }),
    };
    const steps = recentSteps(tools, 0, 3);
    expect(steps.map((s) => [s.callId, s.state])).toEqual([
      ["b", "running"],
      ["d", "failed"],
      ["a", "done"],
    ]);
    expect(steps[0].label).toBe("Running npm test");
  });

  it("only counts steps of the current turn", () => {
    expect(recentSteps({ a: tool({ startedAt: 1 }) }, 5, 3)).toEqual([]);
  });
});

describe("islandPlanRow", () => {
  const todos = {
    main: [
      { content: "A", activeForm: "Doing A", status: "completed" as const },
      { content: "B", activeForm: "Doing B", status: "in_progress" as const },
    ],
  };

  it("reads the attached plan's tasks over the todos", () => {
    const plan = work([task("a", { status: { type: "done" } }), task("b", { title: "Replay on reconnect", status: { type: "active" } })]);
    expect(islandPlanRow({ sessionId: "S1", todos, work: plan })).toEqual({ done: 1, total: 2, label: "1 of 2 · Replay on reconnect" });
  });

  it("falls back to the main agent's todos, and to nothing", () => {
    expect(islandPlanRow({ sessionId: "S1", todos, work: null })).toEqual({ done: 1, total: 2, label: "1 of 2 · Doing B" });
    expect(islandPlanRow({ sessionId: "S1", todos: {}, work: null })).toBeNull();
    expect(islandPlanRow(null)).toBeNull();
  });
});
