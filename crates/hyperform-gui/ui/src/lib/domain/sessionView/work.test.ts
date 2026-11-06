import { describe, expect, it } from "vitest";
import type { Event } from "../../protocol/Event";
import { snapshot } from "../testFixtures";
import { entry, header, health, task, update } from "../work/fixtures";
import { reduce } from "./reduce";
import { emptyView, type SessionView } from "./types";
import { PACKETS_KEPT } from "./work";

function run(events: Event[], start: SessionView = emptyView("S1"), now = 5_000): SessionView {
  return events.reduce((view, event) => reduce(view, event, now), start);
}

const workSnap = (over: Partial<ReturnType<typeof header>> = {}) => ({
  plan: header(over),
  tasks: [task("t1"), task("t2")],
  entries: [],
  findings: [],
  audit: [],
  health: health(),
  replanHeld: [],
});

describe("work in the session view", () => {
  it("stays null without a plan, so the Plan tab looks as it does today", () => {
    const view = run([{ type: "snapshot", snapshot: snapshot() }]);
    expect(view.work).toBeNull();
    expect(view.packets).toEqual([]);
  });

  it("takes the attached plan from the snapshot", () => {
    const view = run([{ type: "snapshot", snapshot: snapshot({ work: workSnap() }) }]);
    expect(view.work?.plan.goal).toBe("Add offline sync support");
    expect(Object.keys(view.work?.tasks ?? {})).toEqual(["t1", "t2"]);
  });

  it("upserts tasks and entries and records live status moves", () => {
    const base = run([{ type: "snapshot", snapshot: snapshot({ work: workSnap() }) }]);
    const plan = header({ status: "active", revision: 2, updatedAt: 3_000 });
    const view = run(
      [
        {
          type: "workUpdated",
          update: update({ plan, tasks: [task("t1", { title: "Renamed", revision: 2 })], entries: [entry("e1", { type: "discovery", text: "x", source: null })] }),
        },
      ],
      base,
    );
    expect(view.work?.tasks.t1.title).toBe("Renamed");
    expect(view.work?.tasks.t2).toBeDefined();
    expect(view.work?.entries.e1).toBeDefined();
    expect(view.work?.moves).toEqual([{ revision: 2, from: "draft", to: "active", at: 3_000 }]);
  });

  it("ignores a late, older revision of the header or a task", () => {
    const base = run([
      { type: "snapshot", snapshot: snapshot({ work: workSnap({ revision: 5 }) }) },
      { type: "workUpdated", update: update({ plan: header({ revision: 6 }), tasks: [task("t1", { title: "New", revision: 6 })] }) },
    ]);
    const late = run(
      [{ type: "workUpdated", update: update({ plan: header({ revision: 4, goal: "Old" }), tasks: [task("t1", { title: "Old", revision: 4 })] }) }],
      base,
    );
    expect(late.work?.plan.revision).toBe(6);
    expect(late.work?.tasks.t1.title).toBe("New");
  });

  it("replaces findings, the audit and health with each revision, but not with a late one", () => {
    const base = run([{ type: "snapshot", snapshot: snapshot({ work: { ...workSnap({ revision: 3 }), health: health({ openFindings: 1 }) } }) }]);
    expect(base.work?.health?.openFindings).toBe(1);
    const finding = {
      id: "taskWithoutCriteria:t1",
      kind: "taskWithoutCriteria" as const,
      severity: "medium" as const,
      text: "Task has no acceptance criteria",
      refs: [],
      suggestion: null,
      status: "open" as const,
      source: "deterministic" as const,
      createdRevision: 4,
      revision: 4,
    };
    const next = run(
      [{ type: "workUpdated", update: update({ plan: header({ revision: 4 }), audit: [finding], health: health({ openFindings: 2 }) }) }],
      base,
    );
    expect(next.work?.audit).toEqual([finding]);
    expect(next.work?.health?.openFindings).toBe(2);
    const late = run([{ type: "workUpdated", update: update({ plan: header({ revision: 2 }), audit: [], health: health() }) }], next);
    expect(late.work?.audit).toEqual([finding]);
    expect(late.work?.health?.openFindings).toBe(2);
  });

  it("takes the tasks held for a plan change from the snapshot and each newer revision", () => {
    const base = run([{ type: "snapshot", snapshot: snapshot({ work: { ...workSnap({ revision: 3 }), replanHeld: ["t2"] } }) }]);
    expect(base.work?.replanHeld).toEqual(["t2"]);
    const accepted = run([{ type: "workUpdated", update: update({ plan: header({ revision: 4 }), replanHeld: [] }) }], base);
    expect(accepted.work?.replanHeld).toEqual([]);
    const late = run([{ type: "workUpdated", update: update({ plan: header({ revision: 2 }), replanHeld: ["t1"] }) }], accepted);
    expect(late.work?.replanHeld).toEqual([]);
  });

  it("starts over when a different plan is attached", () => {
    const base = run([{ type: "snapshot", snapshot: snapshot({ work: workSnap() }) }]);
    const other = header({ planId: "P2", goal: "Other" });
    const view = run([{ type: "workUpdated", update: update({ plan: other, tasks: [task("x")] }) }], base);
    expect(view.work?.plan.planId).toBe("P2");
    expect(Object.keys(view.work?.tasks ?? {})).toEqual(["x"]);
  });

  it("keeps live moves and packets across a snapshot of the same plan, drops them when detached", () => {
    const base = run([
      { type: "snapshot", snapshot: snapshot({ work: workSnap() }) },
      { type: "workUpdated", update: update({ plan: header({ status: "active", revision: 2 }) }) },
      { type: "taskPacketSent", agentId: "main", taskId: "t1", tokens: 900, reason: "compaction" },
    ]);
    const again = run([{ type: "snapshot", snapshot: snapshot({ work: workSnap({ status: "active", revision: 2 }) }) }], base);
    expect(again.work?.moves).toHaveLength(1);
    expect(again.packets).toHaveLength(1);
    const detached = run([{ type: "snapshot", snapshot: snapshot() }], again);
    expect(detached.work).toBeNull();
    expect(detached.packets).toEqual([]);
  });

  it("keeps the newest packets, stamped when they arrived", () => {
    const events: Event[] = Array.from({ length: PACKETS_KEPT + 3 }, (_, i) => ({
      type: "taskPacketSent",
      agentId: "main",
      taskId: `t${i}`,
      tokens: i,
      reason: "task",
    }));
    const view = run(events, emptyView("S1"), 9_000);
    expect(view.packets).toHaveLength(PACKETS_KEPT);
    expect(view.packets.at(-1)).toMatchObject({ taskId: `t${PACKETS_KEPT + 2}`, at: 9_000 });
  });
});
