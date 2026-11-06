import { describe, expect, it } from "vitest";
import type { EventEnvelope } from "../../protocol/EventEnvelope";
import { applyEnvelope, emptySessionsState } from "../sessions";
import { assistant, snapshot, user } from "../testFixtures";
import { buildTimeline } from "../timeline/turns";
import { entry, header, health, task, work } from "./fixtures";
import { movedTraces, PLAN_MOVED_NOTICE, planRestored, planTraces, restoreTimes } from "./trace";

describe("plan traces", () => {
  it("leaves one line for an approval seen live, a plan change, a failed and a blocked task, oldest first", () => {
    const view = work(
      [
        task("a", { title: "Retry backoff", status: { type: "failed", reason: "tests" }, updatedAt: 400 }),
        task("b", { title: "Badge", status: { type: "blocked", reason: { type: "mergeConflict", detail: "" } }, updatedAt: 500 }),
        task("c", { createdRevision: 8, revision: 8, updatedAt: 300 }),
        task("ok", { status: { type: "done" } }),
      ],
      {
        moves: [
          { revision: 2, from: "draft", to: "active", at: 100 },
          { revision: 7, from: "active", to: "replanning", at: 250 },
        ],
        entries: {
          c1: entry(
            "c1",
            { type: "planChange", cause: "discovery", source: null, original: "", new: "", reason: "", evidence: [], affected: ["c"] },
            { createdAt: 300, createdRevision: 8 },
          ),
        },
      },
    );
    const traces = planTraces(view);
    expect(traces.map((t) => [t.kind, t.text, t.detail])).toEqual([
      ["approved", "Plan approved", "rev 2"],
      ["revised", "Plan revised", "rev 8 · 1 task added"],
      ["failed", "Task failed", "Retry backoff · tests"],
      ["blocked", "Task blocked", "Badge · merge conflict"],
    ]);
    expect(traces[1]).toMatchObject({ entryId: "c1", planId: "P1" });
    expect(traces[2]).toMatchObject({ taskId: "a", tone: "danger" });
  });

  it("leaves one quiet line when the plan resumed after a restart, with its Why", () => {
    const view = work([task("a")], {
      entries: {
        r1: entry(
          "r1",
          {
            type: "planChange",
            cause: "resume",
            source: null,
            original: "",
            new: "",
            reason: "Resumed: 2 tasks were interrupted, 1 file changed outside the plan",
            evidence: [],
            affected: ["a"],
          },
          { createdAt: 700, createdRevision: 9 },
        ),
      },
    });
    expect(planTraces(view)).toEqual([
      {
        key: "trace:resumed:r1",
        planId: "P1",
        kind: "resumed",
        at: 700,
        text: "Plan resumed",
        detail: "2 tasks were interrupted, 1 file changed outside the plan",
        tone: "quiet",
        taskId: null,
        entryId: "r1",
      },
    ]);
  });

  it("keeps a line where the plan moved to another chat, through the snapshot of reopening that chat", () => {
    const messages = [user("u1", "Build it", 10), assistant("a1", "On it", 20)];
    const plan = { plan: header(), tasks: [], entries: [], findings: [], audit: [], health: health(), replanHeld: [] };
    const events: EventEnvelope["event"][] = [
      { type: "snapshot", snapshot: snapshot({ messages, work: plan }) },
      { type: "snapshot", snapshot: snapshot({ messages }) },
      { type: "notice", level: "info", text: PLAN_MOVED_NOTICE },
      { type: "snapshot", snapshot: snapshot({ messages }) },
    ];
    const state = events.reduce(
      (s, event, i) => applyEnvelope(s, { sessionId: "S1", seq: i + 1, event }, "S2", 100 + i),
      emptySessionsState(),
    );
    const view = state.views.S1;
    expect(view.work).toBeNull();
    const traces = movedTraces(view.notices);
    expect(traces).toEqual([expect.objectContaining({ kind: "moved", planId: null, text: "This plan moved to another chat.", at: 102 })]);
    const [turn] = buildTimeline({ messages, traces });
    expect(turn.items.map((i) => i.kind)).toEqual(["text", "trace"]);
    expect(movedTraces([{ id: 1, level: "info", text: "Saved", at: 1 }])).toEqual([]);
  });

  it("names a re-approval after replanning", () => {
    const view = work([], { moves: [{ revision: 9, from: "replanning", to: "active", at: 1 }] });
    expect(planTraces(view)[0].text).toBe("Plan changes accepted");
  });

  it("marks a compaction restored when the main agent got its packet before the next one", () => {
    const times = restoreTimes([
      { agentId: "agt_1", taskId: "t", tokens: 1, reason: "compaction", at: 150 },
      { agentId: "main", taskId: "t", tokens: 1, reason: "compaction", at: 120 },
      { agentId: "main", taskId: "t", tokens: 1, reason: "turn", at: 110 },
    ]);
    expect(times).toEqual([120]);
    expect(planRestored(100, 200, times)).toBe(true);
    expect(planRestored(130, null, times)).toBe(false);
    expect(planRestored(0, 100, times)).toBe(false);
  });
});
