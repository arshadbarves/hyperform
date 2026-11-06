import { describe, expect, it } from "vitest";
import { DONE_FLASH_MS, freshFinish, liveStatus, waitingChats, type LiveStatusInput } from "./liveStatus";
import { emptyView, type SessionView, type ToolCallView } from "./sessionView/types";
import { agentInfo, approval, jobInfo, turnRecord } from "./testFixtures";
import { task, work } from "./work/fixtures";

function tool(over: Partial<ToolCallView> = {}): ToolCallView {
  return {
    callId: "c1",
    agentId: "main",
    tool: "Read",
    title: "Read src/auth.rs",
    input: { file_path: "src/auth.rs" },
    status: "running",
    summary: "",
    output: "",
    progress: "",
    durationMs: null,
    startedAt: 1000,
    ...over,
  };
}

function view(over: Partial<SessionView> = {}): SessionView {
  return { ...emptyView("S1"), ...over };
}

function input(over: Partial<LiveStatusInput> = {}): LiveStatusInput {
  return { view: view(), waiting: [], notice: null, flashTurnId: null, awaySince: null, now: 10_000, ...over };
}

const busy = (over: Partial<SessionView> = {}) =>
  view({
    status: "busy",
    costUsd: 0.5,
    activeTurn: { turnId: "t1", messageId: "u1", startedAt: 2_000, costAtStart: 0.42 },
    ...over,
  });

describe("liveStatus", () => {
  it("is quiet when idle and shows the session cost once there is one", () => {
    expect(liveStatus(input())).toMatchObject({ kind: "idle", text: null, cost: null, waiting: null });
    expect(liveStatus(input({ view: view({ costUsd: 1.5 }) })).cost).toBe("$1.50");
  });

  it("puts this chat's approval above everything else", () => {
    const state = liveStatus(
      input({ view: busy({ approvals: { r1: approval() }, tools: { c1: tool() } }), notice: { id: 1, text: "Saved", tone: "ok" } }),
    );
    expect(state).toMatchObject({ kind: "attention", activity: "approval", text: "Needs your approval", detail: "Run cargo test" });
  });

  it("lets a passing notice interrupt work, then work resumes", () => {
    const working = busy({ tools: { c1: tool() } });
    const notice = { id: 7, text: "Could not start a chat · offline", title: "Could not start a chat", tag: "offline", tone: "warn" as const };
    expect(liveStatus(input({ view: working, notice }))).toMatchObject({
      kind: "notice",
      activity: "warn",
      text: "Could not start a chat",
      detail: "offline",
      noticeId: 7,
    });
    expect(liveStatus(input({ view: working })).kind).toBe("working");
  });

  it("names the main agent's current tool with the turn's time and cost", () => {
    const tools = {
      c1: tool({ startedAt: 3_000 }),
      c2: tool({ callId: "c2", tool: "Bash", input: { command: "npm test", description: "Run the ui tests" }, startedAt: 4_000 }),
      c3: tool({ callId: "c3", agentId: "agt_1", tool: "Grep", startedAt: 5_000 }),
    };
    expect(liveStatus(input({ view: busy({ tools }) }))).toMatchObject({
      kind: "working",
      activity: "bash",
      text: "Running the ui tests +1",
      elapsed: "8s",
      cost: "$0.080",
    });
  });

  it("falls back to thinking, writing and checking", () => {
    const stream = (text: string) => ({ m1: { messageId: "m1", agentId: "main", text, thinking: "hmm", startedAt: 1 } });
    expect(liveStatus(input({ view: busy({ streaming: stream("") }) })).text).toBe("Thinking");
    expect(liveStatus(input({ view: busy({ streaming: stream("Here") }) })).text).toBe("Writing a reply");
    const checking = busy({ verification: { status: "unverified", reason: "running" } });
    expect(liveStatus(input({ view: checking })).text).toBe("Checking the changes");
  });

  it("says the context is being compacted, over any leftover step", () => {
    const compacting = busy({ compacting: true, tools: { c1: tool() } });
    expect(liveStatus(input({ view: compacting }))).toMatchObject({
      kind: "working",
      activity: "compact",
      text: "Compacting context",
      elapsed: "8s",
    });
    const manual = view({ status: "busy", compacting: true });
    expect(liveStatus(input({ view: manual }))).toMatchObject({ activity: "compact", elapsed: null });
    expect(liveStatus(input({ view: busy({ compacting: true, approvals: { r1: approval() } }) })).kind).toBe(
      "attention",
    );
  });

  it("shows plan progress only while working", () => {
    const todos = {
      main: [
        { content: "A", activeForm: "Doing A", status: "completed" as const },
        { content: "B", activeForm: "Doing B", status: "in_progress" as const },
      ],
    };
    expect(liveStatus(input({ view: busy({ todos }) })).progress).toEqual({ done: 1, total: 2 });
    expect(liveStatus(input({ view: view({ todos }) })).progress).toBeNull();
  });

  it("with a plan attached, counts tasks and names the current task's step over the tool", () => {
    const replay = task("r", {
      status: { type: "active" },
      owner: { sessionId: "S1", agentId: "main" },
      steps: [{ content: "Run the sync tests", activeForm: "Running the sync tests", status: "in_progress" }],
    });
    const plan = work([task("a", { status: { type: "done" } }), replay, task("c")]);
    expect(liveStatus(input({ view: busy({ work: plan, tools: { c1: tool() } }) }))).toMatchObject({
      progress: { done: 1, total: 3 },
      text: "Running the sync tests",
      detail: "Reading auth.rs",
      activity: "read",
    });
    expect(liveStatus(input({ view: busy({ work: plan, compacting: true }) })).text).toBe("Compacting context");
  });

  it("counts down a provider retry", () => {
    const retrying = { attempt: 2, delayMs: 5_000, reason: "overloaded", at: 8_000 };
    expect(liveStatus(input({ view: busy({ retrying }) }))).toMatchObject({ kind: "retrying", detail: "retry 2 in 3s" });
  });

  it("announces the flashing turn's result with its duration and cost", () => {
    const verified = view({
      turns: [turnRecord({ verification: { status: "verified", checks: [] }, finishedAt: 62_000, costUsd: 0.12 })],
    });
    expect(liveStatus(input({ view: verified, flashTurnId: "t1" }))).toMatchObject({
      kind: "done",
      tone: "ok",
      text: "Verified",
      elapsed: "1m 02s",
      cost: "$0.120",
    });
    const failed = view({ turns: [turnRecord({ outcome: { type: "failed", message: "boom" } })] });
    expect(liveStatus(input({ view: failed, flashTurnId: "t1" }))).toMatchObject({ tone: "danger", text: "Failed" });
    expect(liveStatus(input({ view: verified })).kind).toBe("idle");
  });

  it("recaps turns that ended while the user was away", () => {
    const turns = [
      turnRecord({ turnId: "t0", finishedAt: 500, costUsd: 0.5 }),
      turnRecord({ turnId: "t1", finishedAt: 5_000, costUsd: 0.1 }),
      turnRecord({ turnId: "t2", finishedAt: 6_000, costUsd: 0.2, verification: { status: "verified", checks: [] } }),
    ];
    expect(liveStatus(input({ view: view({ turns }), awaySince: 1_000 }))).toMatchObject({
      kind: "recap",
      tone: "ok",
      text: "While you were away: Verified",
      detail: "2 turns finished",
      cost: "$0.300",
    });
    expect(liveStatus(input({ view: view({ turns }), awaySince: 7_000 })).kind).toBe("idle");
  });

  it("reports another waiting chat beside whatever this one is doing", () => {
    const waiting = [
      { sessionId: "S2", title: "Fix CI" },
      { sessionId: "S3", title: "Docs" },
    ];
    expect(liveStatus(input({ waiting }))).toMatchObject({ kind: "idle", waiting: waiting[0], moreWaiting: 1 });
    expect(liveStatus(input({ waiting, view: busy() }))).toMatchObject({ kind: "working", waiting: waiting[0] });
  });

  it("counts helpers for the orbiting sprites", () => {
    const agents = { agt_1: agentInfo(), agt_2: agentInfo({ agentId: "agt_2", status: "completed" }) };
    const jobs = { job_1: jobInfo() };
    expect(liveStatus(input({ view: view({ agents, jobs }) })).helpers).toEqual({ running: 2, pending: 0 });
  });
});

describe("freshFinish", () => {
  const done = view({ turns: [turnRecord({ finishedAt: 10_000 })] });

  it("returns the last turn only inside the flash window", () => {
    expect(freshFinish(done, 10_000 + DONE_FLASH_MS - 1)).toEqual({ turnId: "t1", remainingMs: 1 });
    expect(freshFinish(done, 10_000 + DONE_FLASH_MS)).toBeNull();
    expect(freshFinish({ ...done, status: "busy" }, 10_001)).toBeNull();
    expect(freshFinish(null, 0)).toBeNull();
  });
});

describe("waitingChats", () => {
  it("lists other chats that wait for an answer", () => {
    const activity = { S1: "approval" as const, S2: "approval" as const, S3: "working" as const };
    expect(waitingChats(activity, "S1", (id) => `chat ${id}`)).toEqual([{ sessionId: "S2", title: "chat S2" }]);
  });
});
