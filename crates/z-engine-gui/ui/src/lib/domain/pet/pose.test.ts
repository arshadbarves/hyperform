import { describe, expect, it } from "vitest";
import {
  ASLEEP_MS,
  BORED_MS,
  GREETING_MS,
  LONG_TASK_MS,
  NO_CONTEXT,
  SLEEPY_MS,
  TOOL_ERROR_MS,
  TYPING_MS,
  petPose,
  poseContext,
  type UserSignals,
} from "./pose";
import type { LiveActivity, LiveStatus } from "../liveStatus";
import { emptyUsage, emptyView } from "../sessionView/types";
import type { TurnRecord } from "../../protocol/TurnRecord";

const NOW = 1_000_000;

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

function signals(over: Partial<UserSignals> = {}): UserSignals {
  return { typingAt: null, scrolledBack: false, hovering: false, lastActiveAt: NOW, returnedAt: null, ...over };
}

const working = (activity: LiveStatus["activity"]) => status({ kind: "working", tone: "working", activity });
const mood = (s: LiveStatus, ctx = NO_CONTEXT) => petPose(s, signals(), "calm", NOW, ctx)?.mood;

describe("petPose: the agent's work", () => {
  it("gives every kind of work its own mood", () => {
    const expected: [LiveActivity, string][] = [
      ["think", "thinking"],
      ["reply", "talking"],
      ["compact", "tidying"],
      ["read", "reading"],
      ["search", "searching"],
      ["web", "searching"],
      ["verify", "searching"],
      ["edit", "coding"],
      ["write", "coding"],
      ["todo", "planning"],
      ["plan", "presenting"],
      ["question", "asking"],
      ["bash", "busy"],
      ["job", "busy"],
      ["mcp", "busy"],
      ["agent", "busy"],
      ["generic", "busy"],
    ];
    for (const [activity, name] of expected) expect([activity, mood(working(activity))]).toEqual([activity, name]);
    expect(petPose(working("read"), signals(), "calm", NOW)).toMatchObject({ gaze: "scan", tone: "working" });
    expect(petPose(working("think"), signals(), "calm", NOW)?.gaze).toBe("up");
  });

  it("gets determined on a long task but keeps its tools", () => {
    const long = { ...NO_CONTEXT, turnStartedAt: NOW - LONG_TASK_MS - 1 };
    expect(mood(working("bash"), long)).toBe("determined");
    expect(mood(working("think"), long)).toBe("determined");
    expect(mood(working("read"), long)).toBe("reading");
    expect(mood(working("compact"), long)).toBe("tidying");
    expect(mood(working("bash"), { ...long, turnStartedAt: NOW - 1000 })).toBe("busy");
  });

  it("is confused for a moment after a tool fails", () => {
    const ctx = { ...NO_CONTEXT, toolErrorAt: NOW - 500 };
    expect(mood(working("bash"), ctx)).toBe("confused");
    expect(mood(working("bash"), { ...ctx, toolErrorAt: NOW - TOOL_ERROR_MS - 1 })).toBe("busy");
    expect(mood(working("compact"), ctx)).toBe("tidying");
  });
});

describe("petPose: needing you and outcomes", () => {
  it("tells a question, an approval and a plan apart", () => {
    const needs = (activity: LiveActivity) => status({ kind: "attention", tone: "attention", activity });
    expect(petPose(needs("question"), signals(), "calm", NOW)).toMatchObject({ mood: "asking", tone: "attention" });
    expect(mood(needs("approval"))).toBe("pleading");
    expect(mood(needs("plan"))).toBe("presenting");
  });

  it("is proud of a verified turn, relieved after a failure, sad or worried otherwise", () => {
    const verified = status({ kind: "done", tone: "ok", activity: "done" });
    expect(mood(verified)).toBe("proud");
    expect(mood(verified, { ...NO_CONTEXT, afterFailure: true })).toBe("relieved");
    expect(mood(status({ kind: "done", tone: "danger", activity: "failed" }))).toBe("sad");
    expect(mood(status({ kind: "done", tone: "quiet", activity: "done" }))).toBe("content");
    expect(mood(status({ kind: "notice", tone: "ok", activity: "done" }))).toBe("happy");
    expect(mood(status({ kind: "notice", tone: "attention", activity: "warn" }))).toBe("worried");
    expect(mood(status({ kind: "retrying", tone: "attention", activity: "retry" }))).toBe("worried");
  });
});

describe("petPose: you and idle time", () => {
  it("reacts to the user only when lively and the agent is idle", () => {
    const typing = signals({ typingAt: NOW - TYPING_MS + 1 });
    expect(petPose(status(), typing, "lively", NOW)).toMatchObject({ mood: "listening", gaze: "down" });
    expect(petPose(status(), typing, "calm", NOW)?.mood).toBe("idle");
    expect(petPose(status(), signals({ scrolledBack: true }), "lively", NOW)).toMatchObject({ mood: "curious", gaze: "up" });
    expect(petPose(status(), signals({ returnedAt: NOW - GREETING_MS + 1 }), "lively", NOW)?.mood).toBe("greeting");
  });

  it("gets bored, then sleepy, then falls asleep, and wakes when hovered", () => {
    const quiet = (ms: number) => signals({ lastActiveAt: NOW - ms - 1 });
    expect(petPose(status(), quiet(BORED_MS), "lively", NOW)?.mood).toBe("bored");
    expect(petPose(status(), quiet(SLEEPY_MS), "lively", NOW)?.mood).toBe("sleepy");
    expect(petPose(status(), quiet(ASLEEP_MS), "lively", NOW)?.mood).toBe("asleep");
    expect(petPose(status(), { ...quiet(ASLEEP_MS), hovering: true }, "lively", NOW)).toMatchObject({ mood: "idle", gaze: "pointer" });
  });

  it("counts the agent's work as company", () => {
    const ctx = { ...NO_CONTEXT, workedAt: NOW - 1000 };
    expect(petPose(status(), signals({ lastActiveAt: NOW - ASLEEP_MS - 1 }), "lively", NOW, ctx)?.mood).toBe("idle");
  });

  it("keeps working but glances at the pointer or the composer", () => {
    expect(petPose(working("bash"), signals({ hovering: true }), "lively", NOW)).toMatchObject({ mood: "busy", gaze: "pointer" });
    expect(petPose(working("bash"), signals({ typingAt: NOW }), "lively", NOW)?.gaze).toBe("down");
    expect(petPose(working("bash"), signals({ hovering: true }), "calm", NOW)?.gaze).toBe("center");
  });

  it("looks toward a waiting chat, and disappears when off", () => {
    const waiting = status({ waiting: { sessionId: "S2", title: "Fix CI" } });
    expect(petPose(waiting, signals(), "calm", NOW)).toMatchObject({ mood: "curious", gaze: "left", tone: "attention" });
    expect(petPose(working("bash"), signals(), "off", NOW)).toBeNull();
  });
});

describe("poseContext", () => {
  const turn = (over: Partial<TurnRecord>): TurnRecord => ({
    turnId: "t",
    messageId: "m",
    outcome: { type: "completed" },
    verification: { status: "verified", checks: [] },
    usage: emptyUsage(),
    costUsd: 0,
    startedAt: 0,
    finishedAt: 10,
    ...over,
  });

  it("knows when the turn started, whether a tool failed, and whether the last turn failed", () => {
    const view = emptyView("s");
    view.activeTurn = { turnId: "t2", messageId: "m2", startedAt: 100, costAtStart: 0 };
    view.turns = [turn({ verification: { status: "failed", reason: "tests" } }), turn({ finishedAt: 50 })];
    view.tools = {
      a: { callId: "a", agentId: "main", tool: "Bash", title: "", input: null, status: "error", summary: "", output: "", progress: "", durationMs: 20, startedAt: 200 },
      b: { callId: "b", agentId: "sub", tool: "Bash", title: "", input: null, status: "error", summary: "", output: "", progress: "", durationMs: 20, startedAt: 900 },
    };
    expect(poseContext(view)).toEqual({ turnStartedAt: 100, afterFailure: true, toolErrorAt: 220, workedAt: 50, turnTone: null });
    expect(poseContext(null)).toEqual(NO_CONTEXT);
  });

  it("carries the judged tone of the latest turn", () => {
    const view = { ...emptyView("s"), turns: [turn({ turnId: "t1" })], turnTones: { t1: "struggling" as const } };
    expect(poseContext(view).turnTone).toBe("struggling");
  });
});

describe("petPose: a judged turn tone (decisions_pet_mood)", () => {
  it("colors a finished turn, and the event-based mood stays without one", () => {
    const verified = status({ kind: "done", tone: "ok", activity: "done" });
    const judged = (turnTone: "smooth" | "struggling" | "blocked" | "done_well") => ({ ...NO_CONTEXT, turnTone });
    expect(mood(verified, judged("struggling"))).toBe("relieved");
    expect(mood(verified, judged("smooth"))).toBe("happy");
    expect(mood(verified, judged("blocked"))).toBe("proud");
    expect(mood(status({ kind: "done", tone: "danger", activity: "failed" }), judged("done_well"))).toBe("sad");
    expect(mood(status({ kind: "notice", tone: "ok", activity: "done" }), judged("struggling"))).toBe("happy");
  });
});
