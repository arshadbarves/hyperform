import { describe, expect, it } from "vitest";
import type { Event } from "../../protocol/Event";
import {
  agentInfo,
  approval,
  assistant,
  checkRecord,
  info,
  jobInfo,
  snapshot,
  text,
  toolResult,
  toolUse,
  turnRecord,
  usage,
  user,
} from "../testFixtures";
import { reduce } from "./reduce";
import { emptyView, PROGRESS_TAIL_CHARS, type SessionView } from "./types";

function run(events: Event[], start: SessionView = emptyView("S1")): SessionView {
  return events.reduce((view, event, i) => reduce(view, event, 1000 + i), start);
}

describe("snapshot", () => {
  it("replaces the view with normalized records", () => {
    const view = run([
      {
        type: "snapshot",
        snapshot: snapshot({
          info: info({ title: "Fix login", mode: "bypass", planning: true, effort: "high" }),
          status: "waiting",
          messages: [user("u1", "hi")],
          todos: [{ content: "a", activeForm: "doing a", status: "pending" }],
          agents: [agentInfo()],
          jobs: [jobInfo()],
          pendingApprovals: [approval()],
          pendingQuestions: [{ requestId: "req_q", agentId: "main", questions: [] }],
          pendingPlans: [{ requestId: "req_p", agentId: "main", plan: "# Plan" }],
          queued: ["later"],
          contextTokens: 1200,
          costUsd: 0.5,
        }),
      },
    ]);
    expect(view.info?.title).toBe("Fix login");
    expect(view.title).toBe("Fix login");
    expect(view.mode).toBe("bypass");
    expect(view.planning).toBe(true);
    expect(view.effort).toBe("high");
    expect(view.status).toBe("waiting");
    expect(view.messages.map((m) => m.id)).toEqual(["u1"]);
    expect(view.todos.main).toHaveLength(1);
    expect(Object.keys(view.agents)).toEqual(["agt_1"]);
    expect(Object.keys(view.jobs)).toEqual(["job_1"]);
    expect(Object.keys(view.approvals)).toEqual(["req_1"]);
    expect(Object.keys(view.questions)).toEqual(["req_q"]);
    expect(view.plans.req_p.plan).toBe("# Plan");
    expect(view.queue).toEqual(["later"]);
    expect(view.contextTokens).toBe(1200);
    expect(view.costUsd).toBe(0.5);
  });

  it("keeps live tool state for calls still in the transcript", () => {
    const before = run([
      { type: "toolStarted", agentId: "main", callId: "c1", tool: "Bash", title: "Run ls", input: {} },
      {
        type: "toolFinished",
        agentId: "main",
        callId: "c1",
        status: "ok",
        summary: "3 files",
        output: "a\nb\nc",
        durationMs: 40,
      },
      { type: "toolStarted", agentId: "main", callId: "gone", tool: "Read", title: "", input: {} },
      { type: "commandOutput", name: "cost", markdown: "$1" },
    ]);
    const after = reduce(
      before,
      {
        type: "snapshot",
        snapshot: snapshot({ messages: [assistant("a1", [toolUse("c1", "Bash")])] }),
      },
      2000,
    );
    expect(after.tools.c1.summary).toBe("3 files");
    expect(after.tools.gone).toBeUndefined();
    // anchored before any message: survives
    expect(after.outputs).toHaveLength(1);
  });

  it("drops in-flight streams when the snapshot says idle", () => {
    const streaming = run([{ type: "textDelta", agentId: "main", messageId: "m9", text: "par" }]);
    const idle = reduce(streaming, { type: "snapshot", snapshot: snapshot() }, 5);
    expect(idle.streaming).toEqual({});
    const busy = reduce(streaming, { type: "snapshot", snapshot: snapshot({ status: "busy" }) }, 5);
    expect(busy.streaming.m9.text).toBe("par");
  });

  it("keeps turn starts for messages that survive a rewind", () => {
    const view = run([
      { type: "turnStarted", turnId: "t1", messageId: "u1" },
      {
        type: "snapshot",
        snapshot: snapshot({ messages: [user("u1", "hi")], turns: [turnRecord({ turnId: "t0", messageId: "u0" })] }),
      },
    ]);
    expect(view.turnStarts).toEqual({ u1: "t1", u0: "t0" });
  });
});

describe("streaming", () => {
  it("accumulates deltas and swaps in the final main message", () => {
    const view = run([
      { type: "assistantStarted", agentId: "main", messageId: "a1" },
      { type: "thinkingDelta", agentId: "main", messageId: "a1", text: "hmm " },
      { type: "textDelta", agentId: "main", messageId: "a1", text: "Hel" },
      { type: "textDelta", agentId: "main", messageId: "a1", text: "lo" },
    ]);
    expect(view.streaming.a1).toMatchObject({ text: "Hello", thinking: "hmm ", agentId: "main" });
    const done = reduce(view, { type: "assistantFinished", agentId: "main", message: assistant("a1", "Hello") }, 9);
    expect(done.streaming).toEqual({});
    expect(done.messages.map((m) => m.id)).toEqual(["a1"]);
  });

  it("creates the entry when a delta arrives first", () => {
    const view = run([{ type: "textDelta", agentId: "main", messageId: "a1", text: "x" }]);
    expect(view.streaming.a1.text).toBe("x");
  });

  it("keeps subagent messages out of the main transcript", () => {
    const view = run([
      { type: "assistantStarted", agentId: "agt_1", messageId: "s1" },
      { type: "textDelta", agentId: "agt_1", messageId: "s1", text: "sub" },
      { type: "assistantFinished", agentId: "agt_1", message: assistant("s1", "sub") },
    ]);
    expect(view.messages).toEqual([]);
    expect(view.streaming).toEqual({});
  });

  it("replaces rather than duplicates a re-sent message", () => {
    const view = run([
      { type: "assistantFinished", agentId: "main", message: assistant("a1", "one") },
      { type: "assistantFinished", agentId: "main", message: assistant("a1", "two") },
    ]);
    expect(view.messages).toHaveLength(1);
    expect(view.messages[0].content).toEqual([text("two")]);
  });
});

describe("turns", () => {
  it("registers turn starts from user messages and turnStarted", () => {
    const view = run([
      { type: "userMessage", message: user("u1", "go"), turnId: "t1", steering: false },
      { type: "turnStarted", turnId: "t1", messageId: "u1" },
      { type: "userMessage", message: user("r1", [toolResult("c1", "ok")]), turnId: "t1", steering: false },
      { type: "userMessage", message: user("s1", "also this"), turnId: "t1", steering: true },
    ]);
    expect(view.messages.map((m) => m.id)).toEqual(["u1", "r1", "s1"]);
    expect(view.turnStarts).toEqual({ u1: "t1" });
    expect(view.steeringIds).toEqual({ s1: true });
    expect(view.activeTurn).toMatchObject({ turnId: "t1", messageId: "u1" });
  });

  it("remembers the session cost a turn started from", () => {
    const view = run([{ type: "turnStarted", turnId: "t1", messageId: "u1" }], { ...emptyView("S1"), costUsd: 0.42 });
    expect(view.activeTurn?.costAtStart).toBe(0.42);
  });

  it("settles the main agent when the turn finishes", () => {
    const view = run([
      { type: "turnStarted", turnId: "t1", messageId: "u1" },
      { type: "toolStarted", agentId: "main", callId: "c1", tool: "Bash", title: "", input: {} },
      { type: "toolStarted", agentId: "agt_1", callId: "c2", tool: "Bash", title: "", input: {} },
      { type: "textDelta", agentId: "main", messageId: "a1", text: "partial" },
      { type: "verificationChanged", outcome: { status: "unverified", reason: "running" } },
      { type: "turnFinished", turn: turnRecord({ outcome: { type: "cancelled" } }) },
    ]);
    expect(view.activeTurn).toBeNull();
    expect(view.turns).toHaveLength(1);
    expect(view.tools.c1.status).toBe("cancelled");
    expect(view.tools.c2.status).toBe("running");
    expect(view.streaming).toEqual({});
    expect(view.verification).toBeNull();
  });

  it("clears the active turn and retry banner on idle", () => {
    const view = run([
      { type: "turnStarted", turnId: "t1", messageId: "u1" },
      { type: "retrying", attempt: 2, delayMs: 4000, reason: "overloaded" },
      { type: "statusChanged", status: "idle" },
    ]);
    expect(view.status).toBe("idle");
    expect(view.activeTurn).toBeNull();
    expect(view.retrying).toBeNull();
  });
});

describe("tools", () => {
  it("tracks a call from start to finish", () => {
    const view = run([
      { type: "toolStarted", agentId: "main", callId: "c1", tool: "Bash", title: "Run tests", input: { command: "npm t" } },
      { type: "toolProgress", agentId: "main", callId: "c1", text: "line 1\n" },
      { type: "toolProgress", agentId: "main", callId: "c1", text: "line 2\n" },
    ]);
    expect(view.tools.c1).toMatchObject({ status: "running", progress: "line 1\nline 2\n", startedAt: 1000 });
    const done = reduce(
      view,
      {
        type: "toolFinished",
        agentId: "main",
        callId: "c1",
        status: "error",
        summary: "exit 1",
        output: "boom",
        durationMs: 812,
      },
      5,
    );
    expect(done.tools.c1).toMatchObject({ status: "error", summary: "exit 1", output: "boom", durationMs: 812 });
    expect(done.tools.c1.progress).toBe("line 1\nline 2\n");
  });

  it("caps the live tail at a line boundary", () => {
    const line = `${"x".repeat(99)}\n`;
    const view = run(
      Array.from({ length: 120 }, (): Event => ({ type: "toolProgress", agentId: "main", callId: "c1", text: line })),
    );
    const tail = view.tools.c1.progress;
    expect(tail.length).toBeLessThanOrEqual(PROGRESS_TAIL_CHARS);
    expect(tail.startsWith("x")).toBe(true);
    expect(tail.endsWith("\n")).toBe(true);
  });
});

describe("interactions and records", () => {
  it("adds and resolves approvals, questions and plans", () => {
    const view = run([
      { type: "approvalRequested", request: approval() },
      { type: "questionAsked", requestId: "q1", agentId: "main", questions: [] },
      { type: "planProposed", requestId: "p1", agentId: "main", plan: "Step 1" },
    ]);
    expect(Object.keys(view.approvals)).toEqual(["req_1"]);
    const resolved = run(
      [
        { type: "approvalResolved", requestId: "req_1", allowed: true },
        { type: "questionResolved", requestId: "q1", answered: false },
        { type: "planResolved", requestId: "p1", approved: true },
      ],
      view,
    );
    expect(resolved.approvals).toEqual({});
    expect(resolved.questions).toEqual({});
    expect(resolved.plans).toEqual({});
  });

  it("keeps todos, agents and jobs keyed by id", () => {
    const view = run([
      { type: "todosUpdated", agentId: "agt_1", todos: [{ content: "x", activeForm: "x-ing", status: "in_progress" }] },
      { type: "agentStarted", info: agentInfo() },
      { type: "agentUpdated", info: agentInfo({ status: "completed" }) },
      { type: "jobUpdated", job: jobInfo({ status: "killed" }) },
      { type: "checkRecorded", record: checkRecord() },
      { type: "checkRecorded", record: checkRecord({ passed: false }) },
      { type: "checkpointCreated", checkpoint: { checkpointId: "k1", messageId: "u1", createdAt: 1 } },
      { type: "checkpointCreated", checkpoint: { checkpointId: "k1", messageId: "u1", createdAt: 1 } },
    ]);
    expect(view.todos.agt_1[0].status).toBe("in_progress");
    expect(view.agents.agt_1.status).toBe("completed");
    expect(view.jobs.job_1.status).toBe("killed");
    expect(view.checks).toHaveLength(1);
    expect(view.checks[0].passed).toBe(false);
    expect(view.checkpoints).toHaveLength(1);
  });

  it("only lets the main agent move the context meter", () => {
    const base = {
      type: "usageUpdated" as const,
      usage: usage({ outputTokens: 5 }),
      sessionUsage: usage({ outputTokens: 50 }),
      costUsd: 0.25,
    };
    const view = run([
      { ...base, agentId: "main", contextTokens: 900, contextLimit: 200_000 },
      { ...base, agentId: "agt_1", contextTokens: 10, contextLimit: 50 },
    ]);
    expect(view.contextTokens).toBe(900);
    expect(view.contextLimit).toBe(200_000);
    expect(view.agentUsage.agt_1.outputTokens).toBe(5);
    expect(view.usage.outputTokens).toBe(50);
    expect(view.costUsd).toBe(0.25);
  });

  it("mirrors mode, planning, model, effort and title into the session info", () => {
    const view = run([
      { type: "snapshot", snapshot: snapshot() },
      { type: "modeChanged", mode: "acceptEdits" },
      { type: "planningChanged", planning: true },
      { type: "modelChanged", model: "openai/gpt-5" },
      { type: "effortChanged", effort: "low" },
      { type: "titleChanged", title: "Refactor" },
      { type: "queueChanged", queued: ["a", "b"] },
    ]);
    expect(view.info).toMatchObject({ mode: "acceptEdits", planning: true, model: "openai/gpt-5", effort: "low", title: "Refactor" });
    expect(view.mode).toBe("acceptEdits");
    expect(view.planning).toBe(true);
    expect(view.queue).toEqual(["a", "b"]);
    const building = run([{ type: "planningChanged", planning: false }], view);
    expect(building).toMatchObject({ mode: "acceptEdits", planning: false });
  });

  it("anchors local cards after the latest message", () => {
    const view = run([
      { type: "userMessage", message: user("u1", "hi"), turnId: null, steering: false },
      { type: "commandOutput", name: "cost", markdown: "**$0.10**" },
      { type: "error", message: "provider down" },
      { type: "notice", level: "warn", text: "careful" },
      { type: "hookRan", hookEvent: "PreToolUse", command: "lint.sh", blocked: true, message: "no" },
      { type: "contextReport", breakdown: { system: 1, tools: 2, instructions: 3, messages: 4, total: 10, limit: 100 } },
    ]);
    expect(view.outputs[0]).toMatchObject({ name: "cost", afterMessageId: "u1" });
    expect(view.errors[0]).toMatchObject({ message: "provider down", afterMessageId: "u1" });
    expect(view.lastError).toBe("provider down");
    expect(view.notices[0]).toMatchObject({ level: "warn", text: "careful" });
    expect(view.hooks[0]).toMatchObject({ hookEvent: "PreToolUse", blocked: true });
    expect(view.contextBreakdown?.total).toBe(10);
    expect(new Set([view.outputs[0].id, view.errors[0].id, view.notices[0].id]).size).toBe(3);
  });

  it("clears the retry banner once the model streams again", () => {
    const view = run([
      { type: "retrying", attempt: 1, delayMs: 2000, reason: "rate limited" },
    ]);
    expect(view.retrying).toMatchObject({ attempt: 1, delayMs: 2000, at: 1000 });
    const resumed = reduce(view, { type: "textDelta", agentId: "main", messageId: "a", text: "x" }, 1);
    expect(resumed.retrying).toBeNull();
  });

  it("ignores unknown events", () => {
    const view = emptyView("S1");
    const next = reduce(view, { type: "fromTheFuture" } as unknown as Event, 1);
    expect(next).toBe(view);
  });
});
