import { describe, expect, it } from "vitest";
import type { TaskRun } from "../../protocol/TaskRun";
import { agentSections, agentTree, workCounts } from "../agentTree";
import { agentInfo } from "../testFixtures";
import { header, task, work } from "./fixtures";
import { awaitsApply, interruptedHelper } from "./interrupted";

const WT = { path: "/repo/.hyperform/worktrees/agt_1", branch: "z/agt_1", base: "main", state: "pending" as const, filesChanged: 2, diffstat: "+9 −1" };
const dispatched = (agentId: string, over: Partial<TaskRun> = {}): TaskRun => ({
  dispatch: { sessionId: "S1", agentId, worktree: WT.path, attempt: 1, at: 1 },
  integration: null,
  report: null,
  forgivenAttempts: 0,
  serial: false,
  ...over,
});
const pointer = { sessionId: "S1", agentId: "agt_1", worktree: WT.path, note: " Converted 14 of 20 files " };
const helper = agentInfo({ agentId: "agt_1", taskId: "t1", status: "cancelled", isolation: "worktree", worktree: WT });

describe("a helper's change its task waits for", () => {
  it("is the current run's, until the change came back or was settled", () => {
    expect(awaitsApply(task("t1", { status: { type: "active" }, run: dispatched("agt_1") }), "agt_1")).toBe(true);
    const parked = dispatched("agt_1", { integration: { type: "pendingApply", worktree: WT.path } });
    expect(awaitsApply(task("t1", { status: { type: "active" }, run: parked }), "agt_1")).toBe(true);
    expect(awaitsApply(task("t1", { status: { type: "active" }, run: dispatched("agt_2") }), "agt_1")).toBe(false);
    expect(awaitsApply(task("t1", { run: dispatched("agt_1") }), "agt_1")).toBe(false);
    const conflict = dispatched("agt_1", { integration: { type: "conflict", paths: ["a.rs"], worktree: WT.path } });
    const blocked = { type: "blocked" as const, reason: { type: "mergeConflict" as const, detail: "a.rs" } };
    expect(awaitsApply(task("t1", { status: blocked, run: conflict }), "agt_1")).toBe(true);
    expect(awaitsApply(task("t1", { status: { type: "active" }, run: dispatched("agt_1", { integration: { type: "applied", files: [] } }) }), "agt_1")).toBe(false);
  });
});

describe("an interrupted task helper", () => {
  it("is kept for the task whose resume pointer names it, with what the plan needs to pick it up", () => {
    const paused = work([task("t1", { title: "Convert settings", resume: pointer })], { plan: header({ status: "paused" }) });
    expect(interruptedHelper(helper, paused)).toMatchObject({
      line: "Interrupted · kept for task “Convert settings”",
      note: "Converted 14 of 20 files",
      hint: "Resume the plan and the task picks up from this work.",
    });
    const active = work([task("t1", { resume: { ...pointer, note: "" } })], { plan: header({ status: "active" }) });
    expect(interruptedHelper(helper, active)).toMatchObject({ note: null, hint: "The task picks up from this work when it runs again." });
  });

  it("is not one while the task waits on its apply, names another helper, or it has no task", () => {
    const waiting = task("t1", { status: { type: "active" }, resume: pointer, run: dispatched("agt_1", { integration: { type: "pendingApply", worktree: WT.path } }) });
    expect(interruptedHelper(helper, work([waiting]))).toBeNull();
    expect(interruptedHelper(helper, work([task("t1", { resume: { ...pointer, agentId: "agt_9" } })]))).toBeNull();
    expect(interruptedHelper(helper, work([task("t1")]))).toBeNull();
    expect(interruptedHelper({ ...helper, taskId: undefined }, work([task("t1", { resume: pointer })]))).toBeNull();
    expect(interruptedHelper(helper, null)).toBeNull();
  });

  it("leaves Ready to apply for its own section and is not counted as ready to apply", () => {
    const ready = agentInfo({ agentId: "agt_2", taskId: "t2", status: "completed", isolation: "worktree", worktree: { ...WT, path: "/w2" }, startedAt: 2 });
    const view = work([
      task("t1", { resume: pointer }),
      task("t2", { status: { type: "active" }, run: dispatched("agt_2", { integration: { type: "pendingApply", worktree: "/w2" } }) }),
    ]);
    const agents = { agt_1: helper, agt_2: ready };
    const sections = agentSections(agentTree(agents), view);
    expect(sections.ready.map((n) => n.info.agentId)).toEqual(["agt_2"]);
    expect(sections.interrupted.map((n) => n.info.agentId)).toEqual(["agt_1"]);
    expect(sections.finished).toEqual([]);
    expect(workCounts(agents, {}, view).pendingWorktrees).toBe(1);
    expect(agentSections(agentTree(agents)).ready).toHaveLength(2);
  });
});
