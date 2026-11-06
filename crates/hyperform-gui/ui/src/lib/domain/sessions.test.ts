import { describe, expect, it } from "vitest";
import {
  activityMap,
  applyEnvelope,
  emptySessionsState,
  eventEffects,
  forgetSession,
  markRead,
  sessionActivity,
} from "./sessions";
import { emptyView } from "./sessionView";
import { approval, envelope, snapshot, turnRecord } from "./testFixtures";
import { header, update } from "./work/fixtures";

describe("applyEnvelope", () => {
  it("creates views on first contact and tracks the sequence", () => {
    const state = applyEnvelope(
      emptySessionsState(),
      envelope({ type: "titleChanged", title: "Hello" }, 0, "A"),
      null,
      1,
    );
    expect(state.views.A.title).toBe("Hello");
    expect(state.views.A.lastSeq).toBe(0);
  });

  it("drops duplicate or stale sequence numbers", () => {
    let state = applyEnvelope(emptySessionsState(), envelope({ type: "queueChanged", queued: ["a"] }, 5), null, 1);
    const stale = applyEnvelope(state, envelope({ type: "queueChanged", queued: ["b"] }, 5), null, 2);
    expect(stale).toBe(state);
    state = applyEnvelope(state, envelope({ type: "queueChanged", queued: ["c"] }, 6), null, 3);
    expect(state.views.S1.queue).toEqual(["c"]);
  });

  it("always applies snapshots and resets the sequence", () => {
    let state = applyEnvelope(emptySessionsState(), envelope({ type: "queueChanged", queued: ["a"] }, 9), null, 1);
    state = applyEnvelope(state, envelope({ type: "snapshot", snapshot: snapshot() }, 3), null, 2);
    expect(state.views.S1.lastSeq).toBe(3);
    expect(state.views.S1.queue).toEqual([]);
  });

  it("marks background turns unread and leaves the active one read", () => {
    const turn = turnRecord({ verification: { status: "verified", checks: ["r1"] } });
    let state = applyEnvelope(emptySessionsState(), envelope({ type: "turnFinished", turn }, 1, "bg"), "fg", 7);
    state = applyEnvelope(state, envelope({ type: "turnFinished", turn }, 1, "fg"), "fg", 8);
    expect(Object.keys(state.unread)).toEqual(["bg"]);
    expect(state.unread.bg).toMatchObject({ outcome: { type: "completed" }, at: 7 });
    expect(markRead(state, "bg").unread).toEqual({});
    expect(markRead(state, "fg")).toBe(state);
  });

  it("forgets deleted sessions", () => {
    const turn = turnRecord();
    const state = applyEnvelope(emptySessionsState(), envelope({ type: "turnFinished", turn }, 1, "bg"), null, 1);
    const next = forgetSession(state, "bg");
    expect(next.views).toEqual({});
    expect(next.unread).toEqual({});
  });
});

describe("activity", () => {
  it("prefers the approval state over working", () => {
    const busy = { ...emptyView("a"), status: "busy" as const };
    const waiting = { ...emptyView("b"), status: "busy" as const, approvals: { r: approval() } };
    expect(sessionActivity(busy)).toBe("working");
    expect(sessionActivity(waiting)).toBe("approval");
    expect(sessionActivity(emptyView("c"))).toBeNull();
    expect(activityMap({ a: busy, b: waiting, c: emptyView("c") })).toEqual({ a: "working", b: "approval" });
  });
});

describe("eventEffects", () => {
  it("toasts notices for the active session only", () => {
    const notice = { type: "notice", level: "warn", text: "slow" } as const;
    expect(eventEffects(notice, true)).toEqual([{ kind: "toast", tone: "warn", text: "slow" }]);
    expect(eventEffects(notice, false)).toEqual([]);
  });

  it("routes shell output to the terminal drawer", () => {
    expect(eventEffects({ type: "commandOutput", name: "shell", markdown: "$ ls" }, true)).toEqual([
      { kind: "shellOutput", text: "$ ls" },
    ]);
    expect(eventEffects({ type: "commandOutput", name: "cost", markdown: "x" }, true)).toEqual([]);
  });

  it("leaves a background request to the title bar instead of a toast", () => {
    expect(eventEffects({ type: "approvalRequested", request: approval() }, false)).toEqual([]);
  });

  it("refreshes the session list when titles or turns change", () => {
    expect(eventEffects({ type: "titleChanged", title: "x" }, false)).toEqual([{ kind: "refreshSessions" }]);
    expect(eventEffects({ type: "hookRan", hookEvent: "Stop", command: "x", blocked: false, message: null }, true)).toEqual([]);
  });

  it("refreshes the project's plan list when a plan changes or a chat opens", () => {
    expect(eventEffects({ type: "workUpdated", update: update({ plan: header({ projectRoot: "/w/app" }) }) }, false)).toEqual([
      { kind: "refreshPlans", projectRoot: "/w/app" },
    ]);
    expect(eventEffects({ type: "snapshot", snapshot: snapshot() }, true)).toEqual([
      { kind: "refreshPlans", projectRoot: snapshot().info.projectRoot },
    ]);
  });

  it("grows the pet from finished turns in any chat", () => {
    const turn = turnRecord({ verification: { status: "verified", checks: ["r1"] } });
    expect(eventEffects({ type: "turnFinished", turn }, false)).toEqual([
      { kind: "refreshSessions" },
      { kind: "petGrowth", signal: { kind: "turn", turnId: turn.turnId, completed: true, verified: true } },
    ]);
    const cancelled = turnRecord({ outcome: { type: "cancelled" } });
    expect(eventEffects({ type: "turnFinished", turn: cancelled }, true)[1]).toMatchObject({
      signal: { completed: false, verified: false },
    });
  });
});
