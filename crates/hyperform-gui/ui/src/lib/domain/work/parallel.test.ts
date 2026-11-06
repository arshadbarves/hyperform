import { describe, expect, it } from "vitest";
import type { ParallelSet } from "../../protocol/ParallelSet";
import { task } from "./fixtures";
import { describeBlock, parallelBadges, parallelCount, workParallelOn } from "./parallel";

const tasks = {
  a: task("a", { title: "Queue writes" }),
  b: task("b", { title: "Status badge" }),
  c: task("c", { title: "Lockfile bump", run: { dispatch: null, integration: null, report: null, forgivenAttempts: 0, serial: true } }),
};

const set = (over: Partial<ParallelSet> = {}): ParallelSet => ({ eligible: [], excluded: [], running: [], max: 3, ...over });

describe("parallel work", () => {
  it("is effective only while work plans are on too", () => {
    expect(workParallelOn({ work_plans: "on", work_parallel: "on" })).toBe(true);
    expect(workParallelOn({ work_plans: "off", work_parallel: "on" })).toBe(false);
    expect(workParallelOn({ work_plans: "on" })).toBe(false);
    expect(workParallelOn({ work_plans: "on", work_parallel: "off" })).toBe(false);
    expect(workParallelOn(null)).toBe(false);
  });

  it("marks eligible tasks only when two or more would run at once", () => {
    expect(parallelBadges(set({ eligible: ["a"] }), tasks)).toEqual({});
    const badges = parallelBadges(set({ eligible: ["a", "b"] }), tasks);
    expect(badges.a).toEqual({
      text: "can run beside others",
      title: "Can run in a parallel helper beside 1 other task",
      together: true,
    });
    expect(parallelBadges(set({ eligible: ["b"], running: ["a"] }), tasks).b?.together).toBe(true);
  });

  it("says why a task runs alone, naming the task that causes it", () => {
    const badges = parallelBadges(
      set({
        excluded: [
          { taskId: "a", block: { type: "unknownScope", taskId: "a" } },
          { taskId: "b", block: { type: "overlap", taskId: "a", files: ["src/sync.rs"], symbols: [] } },
          { taskId: "c", block: { type: "runAlone", taskId: "c" } },
        ],
      }),
      tasks,
    );
    expect(badges.a).toMatchObject({ text: "runs alone", together: false });
    expect(badges.a?.title).toBe("It names no files, so an overlap cannot be ruled out");
    expect(badges.b).toMatchObject({ text: "overlaps Queue writes", title: "It shares src/sync.rs with “Queue writes”" });
    expect(badges.c?.title).toBe("Retried one at a time after a merge conflict");
  });

  it("leaves plan-wide blocks to the plan's own controls", () => {
    const badges = parallelBadges(
      set({
        excluded: [
          { taskId: "a", block: { type: "notApproved", parallel: "ask" } },
          { taskId: "b", block: { type: "planning" } },
          { taskId: "c", block: { type: "replanning" } },
        ],
      }),
      tasks,
    );
    expect(badges).toEqual({});
    expect(describeBlock({ type: "replanning" }, tasks, "c")).toBe("Held until you accept the plan change");
  });

  it("describes every block in words", () => {
    expect(describeBlock({ type: "notApproved", parallel: "ask" }, tasks, "a")).toBe("Parallel helpers wait for your approval");
    expect(describeBlock({ type: "notApproved", parallel: "off" }, tasks, "a")).toBe("Parallel helpers are off for this plan");
    expect(describeBlock({ type: "serialPaths", taskId: "a", patterns: ["Cargo.lock"] }, tasks, "a")).toBe(
      "It touches Cargo.lock, which never change in parallel",
    );
    expect(describeBlock({ type: "runAlone", taskId: "b" }, tasks, "a")).toBe("“Status badge” runs alone, so nothing starts beside it");
    expect(describeBlock({ type: "ordered", taskId: "b" }, tasks, "a")).toBe("A dependency links it to “Status badge”");
    expect(describeBlock({ type: "noSlot", max: 3 }, tasks, "a")).toBe("3 helpers already run, the most at once (work.max_parallel)");
  });

  it("counts what can run together for the Build menu", () => {
    expect(parallelCount(null)).toBe(0);
    expect(parallelCount(set({ eligible: ["a"] }))).toBe(0);
    expect(parallelCount(set({ eligible: ["a", "b"] }))).toBe(2);
  });
});
