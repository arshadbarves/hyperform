import { describe, expect, it } from "vitest";
import { header, task, work } from "./fixtures";
import { holdBadges, holdEdit, ON_HOLD, replanHeldTasks, replanWaitLine, WAITS_FOR_CHANGE } from "./held";

describe("holding tasks", () => {
  it("holds and releases through their own edits", () => {
    expect(holdEdit("t1", true)).toEqual({ type: "holdTask", taskId: "t1", held: true });
    expect(holdEdit("t1", false)).toEqual({ type: "releaseTask", taskId: "t1" });
  });

  it("lists the open tasks the plan change holds, in plan order, only while replanning", () => {
    const tasks = [
      task("a", { order: 2 }),
      task("b", { order: 1 }),
      task("c", { status: { type: "done" } }),
    ];
    const replanning = work(tasks, { plan: header({ status: "replanning" }), replanHeld: ["a", "b", "c", "gone"] });
    expect(replanHeldTasks(replanning).map((t) => t.id)).toEqual(["b", "a"]);
    const active = work(tasks, { plan: header({ status: "active" }), replanHeld: ["a"] });
    expect(replanHeldTasks(active)).toEqual([]);
  });

  it("badges your holds and the plan change's, yours first", () => {
    const view = work(
      [task("a"), task("b", { held: true }), task("c", { held: true, status: { type: "cancelled" } })],
      { plan: header({ status: "replanning" }), replanHeld: ["a", "b"] },
    );
    expect(holdBadges(view)).toEqual({ a: WAITS_FOR_CHANGE, b: ON_HOLD });
  });

  it("says how many tasks wait for the plan change", () => {
    expect(replanWaitLine(0)).toBeNull();
    expect(replanWaitLine(1)).toBe("1 task waits for you to accept the plan change");
    expect(replanWaitLine(3)).toBe("3 tasks wait for you to accept the plan change");
  });
});
