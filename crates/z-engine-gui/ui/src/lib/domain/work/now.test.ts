import { describe, expect, it } from "vitest";
import type { TaskRun } from "../../protocol/TaskRun";
import { agentInfo } from "../testFixtures";
import { criterion, task, work } from "./fixtures";
import { helperTask, laneCriteria, laneStep, laneStopLine, nowLanes } from "./now";

const dispatched = (agentId: string, at: number, worktree: string | null = null): TaskRun => ({
  dispatch: { sessionId: "S1", agentId, worktree, attempt: 1, at },
  integration: null,
  report: null,
  forgivenAttempts: 0,
  serial: false,
});

describe("now lanes", () => {
  it("shows the main agent's task first, then one lane per helper in the order they were sent", () => {
    const view = work([
      task("h2", { status: { type: "active" }, owner: { sessionId: "S1", agentId: "a2" }, run: dispatched("a2", 20) }),
      task("m", { status: { type: "active" }, owner: { sessionId: "S1", agentId: "main" } }),
      task("h1", {
        status: { type: "active" },
        owner: { sessionId: "S1", agentId: "a1" },
        run: dispatched("a1", 10, "/repo/.hyperform/worktrees/a1"),
      }),
      task("idle"),
    ]);
    const agents = { a1: agentInfo({ agentId: "a1" }) };
    const lanes = nowLanes(view, "S1", agents);
    expect(lanes.map((l) => [l.task.id, l.owner])).toEqual([
      ["m", "main"],
      ["h1", "helper"],
      ["h2", "helper"],
    ]);
    expect(lanes[1]).toMatchObject({ agentId: "a1", worktree: "/repo/.hyperform/worktrees/a1" });
    expect(lanes[1].agent?.agentId).toBe("a1");
    expect(lanes[2].agent).toBeNull();
  });

  it("keeps another chat's task and an unowned verifying task, last", () => {
    const view = work([
      task("v", { status: { type: "verifying" } }),
      task("o", { status: { type: "active" }, owner: { sessionId: "S2", agentId: "main" } }),
    ]);
    expect(nowLanes(view, "S1", {}).map((l) => [l.task.id, l.owner])).toEqual([
      ["o", "other"],
      ["v", "none"],
    ]);
  });

  it("says how each lane stops while the plan is paused", () => {
    const view = work([
      task("m", { status: { type: "active" }, owner: { sessionId: "S1", agentId: "main" } }),
      task("h", { status: { type: "active" }, owner: { sessionId: "S1", agentId: "a1" }, run: dispatched("a1", 10, "/w/a1") }),
      task("g", { status: { type: "active" }, owner: { sessionId: "S1", agentId: "a2" }, run: dispatched("a2", 20) }),
      task("o", { status: { type: "active" }, owner: { sessionId: "S2", agentId: "main" } }),
      task("v", { status: { type: "verifying" } }),
    ]);
    const agents = { a1: agentInfo({ agentId: "a1" }), a2: agentInfo({ agentId: "a2", status: "cancelled" }) };
    expect(nowLanes(view, "S1", agents).map((l) => [l.task.id, laneStopLine(l)])).toEqual([
      ["m", "Stops after its current step"],
      ["h", "Stopping at its next step · its worktree is kept"],
      ["g", "Stopped"],
      ["o", "Stopping in another chat"],
      ["v", "Its checks still finish"],
    ]);
  });

  it("carries the task's step and how many criteria are met", () => {
    const view = work([
      task("m", {
        status: { type: "active" },
        owner: { sessionId: "S1", agentId: "main" },
        steps: [
          { content: "Write it", activeForm: "Writing it", status: "completed" },
          { content: "Test it", activeForm: "Running the sync tests", status: "in_progress" },
        ],
      }),
    ]);
    const [lane] = nowLanes(view, "S1", {});
    expect(lane).toMatchObject({ step: "Running the sync tests", steps: { done: 1, total: 2 } });
    expect(laneStep(lane)).toBe("1 of 2 steps · Running the sync tests");
    expect(laneCriteria(lane)).toBeNull();
  });

  it("falls back to what the helper does now, never repeating the title", () => {
    const view = work([
      task("h", {
        title: "Badge",
        activeForm: "Adding the badge",
        status: { type: "active" },
        owner: { sessionId: "S1", agentId: "a1" },
        acceptance: [criterion("c1", { status: "met" }), criterion("c2")],
      }),
      task("t", { title: "Tests", activeForm: "", status: { type: "verifying" } }),
    ]);
    const [lane, bare] = nowLanes(view, "S1", {});
    expect(laneStep(lane)).toBe("Adding the badge");
    expect(laneStep(lane, "Reading src/badge.ts")).toBe("Reading src/badge.ts");
    expect(laneCriteria(lane)).toBe("Criteria 1 of 2 met");
    expect(laneStep(bare)).toBeNull();
  });
});

describe("helper task", () => {
  it("looks the task up by the helper's taskId, else by what was dispatched to it here", () => {
    const view = work([task("t1", { title: "Queue writes" }), task("t2", { title: "Badge", run: dispatched("a2", 1) })]);
    expect(helperTask(agentInfo({ agentId: "a1", taskId: "t1" }), view, "S1")?.title).toBe("Queue writes");
    expect(helperTask(agentInfo({ agentId: "a2" }), view, "S1")?.title).toBe("Badge");
    expect(helperTask(agentInfo({ agentId: "a2" }), view, "S2")).toBeNull();
    expect(helperTask(agentInfo({ agentId: "a1" }), null, "S1")).toBeNull();
  });
});
