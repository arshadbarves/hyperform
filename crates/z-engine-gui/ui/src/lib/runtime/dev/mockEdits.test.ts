import { describe, expect, it } from "vitest";
import { editPlan } from "./mockEdits";
import { settleHelper } from "./mockHelpers";
import { mockPlans } from "./mockPlans";
import { MockWorld } from "./mockWorld";

const plans = () => Object.fromEntries(mockPlans(1_000_000).plans.map((p) => [p.work.plan.planId, p.work]));
const helpers = () => Object.fromEntries(Object.values(mockPlans(1_000_000).agents).flat().map((a) => [a.agentId, a]));

describe("mock backend edits", () => {
  it("moves the plan as the kernel allows and refuses the rest", () => {
    const { "P-build": build, "P-done": done } = plans();
    const paused = editPlan(build, { type: "pause" }, 5);
    expect(typeof paused !== "string" && paused.work.plan.status).toBe("paused");
    expect(editPlan(build, { type: "resume" }, 5)).toMatch(/cannot resume/);
    expect(editPlan(done, { type: "abandon" }, 5)).toMatch(/reopen it first/);
    const reopened = editPlan(done, { type: "reopen" }, 5);
    expect(typeof reopened !== "string" && reopened.work.plan.status).toBe("active");
  });

  it("stops by pausing and releasing the running helpers' tasks, keeping where they were", () => {
    const stopped = editPlan(plans()["P-build"], { type: "stop" }, 5);
    if (typeof stopped === "string") throw new Error(stopped);
    expect(stopped.work.plan.status).toBe("paused");
    const index = stopped.work.tasks.find((t) => t.id === "s2");
    expect(index).toMatchObject({ owner: null, status: { type: "pending" } });
    expect(index?.resume).toMatchObject({ agentId: "agt-index", worktree: expect.stringContaining("index-on-save") });
    expect(stopped.tasks.map((t) => t.id)).toEqual(["s2"]);
  });

  it("holds and releases a task, and accepting the change frees the replan-held ones", () => {
    const { "P-paused": paused, "P-replan": replan } = plans();
    const released = editPlan(paused, { type: "releaseTask", taskId: "m3" }, 5);
    expect(typeof released !== "string" && released.tasks[0].held).toBe(false);
    const accepted = editPlan(replan, { type: "approve" }, 5);
    expect(typeof accepted !== "string" && accepted.work.replanHeld).toEqual([]);
  });
});

describe("mock helpers' Apply and Discard", () => {
  it("brings a waiting change to its task", () => {
    const settled = settleHelper(helpers()["agt-ui"], plans()["P-build"], "applied");
    if (typeof settled === "string") throw new Error(settled);
    expect(settled.info.worktree?.state).toBe("applied");
    expect(settled.tasks.get("s3")).toMatchObject({ status: { type: "done" }, run: { integration: { type: "applied" } } });
  });

  it("refuses Apply for work kept for an interrupted task, and Discard leaves its pointer without the worktree", () => {
    const convert = helpers()["agt-convert"];
    expect(settleHelper(convert, plans()["P-paused"], "applied")).toBe(
      "Could not apply agent agt-convert's changes: This helper's work belongs to task m2. Resume the task or discard its work.",
    );
    const settled = settleHelper(convert, plans()["P-paused"], "discarded");
    if (typeof settled === "string") throw new Error(settled);
    expect(settled.info.worktree?.state).toBe("discarded");
    expect(settled.tasks.get("m2")?.resume).toMatchObject({ agentId: "agt-convert", worktree: null, note: "Converted 14 of 20 files" });
  });

  it("refuses Apply for a task's helper once its chat no longer has the plan", () => {
    expect(settleHelper(helpers()["agt-ui"], null, "applied")).toMatch(/belongs to task s3/);
  });
});

describe("mock world", () => {
  it("lists the project's plans newest first, with the kernel's needs-you counts and the chats they are in", () => {
    const list = new MockWorld(1_000_000).planList("/Users/you/notes-app");
    const plan = (id: string) => list.find((p) => p.planId === id);
    expect(list[0].planId).toBe("P-build");
    expect(plan("P-build")).toMatchObject({ needsYou: 2, attachedTo: "c-build" });
    expect(plan("P-replan")).toMatchObject({ needsYou: 1, lastSessionId: "c-replan", attachedTo: "c-replan" });
    expect(plan("P-docs")).toMatchObject({ lastSessionId: "c-guide", attachedTo: null });
    expect(plan("P-gql")?.lastSessionId).toBeNull();
  });

  it("keeps the replan-held tasks out of the parallel set", () => {
    const set = new MockWorld(1_000_000).parallel("c-replan");
    expect(set.excluded.map((e) => e.taskId)).toEqual(["b3", "b4"]);
    expect(set.eligible).toContain("b5");
  });
});
